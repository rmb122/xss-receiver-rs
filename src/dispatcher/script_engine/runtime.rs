use std::{
    fmt,
    future::{Future, poll_fn},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
    time::{Duration, Instant},
};

use rquickjs::{AsyncContext, AsyncRuntime, CaughtError, Ctx};

#[derive(Debug, Clone)]
pub struct ScriptError(pub(crate) String);

impl ScriptError {
    pub fn from_js(ctx: &Ctx<'_>, error: rquickjs::Error) -> Self {
        Self(CaughtError::from_error(ctx, error).to_string())
    }

    pub fn timeout() -> Self {
        Self("script running timeout".into())
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "js running failed: {}", self.0)
    }
}

impl std::error::Error for ScriptError {}

impl From<rquickjs::Error> for ScriptError {
    fn from(error: rquickjs::Error) -> Self {
        Self(error.to_string())
    }
}

enum JobStep {
    Idle,
    Progress,
    Waiting,
}

async fn step(runtime: &AsyncRuntime) -> Result<JobStep, ScriptError> {
    if !runtime.is_job_pending().await {
        return Ok(JobStep::Idle);
    }
    match runtime.execute_pending_job().await {
        Ok(true) => Ok(JobStep::Progress),
        Ok(false) if runtime.is_job_pending().await => Ok(JobStep::Waiting),
        Ok(false) => Ok(JobStep::Idle),
        Err(error) => Err(error
            .0
            .with(|ctx| ScriptError::from_js(&ctx, rquickjs::Error::Exception))
            .await),
    }
}

/// Drain QuickJS jobs and native futures without polling pending I/O repeatedly.
pub async fn run_jobs(runtime: &AsyncRuntime) -> Result<(), ScriptError> {
    let mut next = Box::pin(step(runtime));
    loop {
        let result = poll_fn(|cx| match next.as_mut().poll(cx) {
            Poll::Ready(Ok(JobStep::Waiting)) => {
                // The native futures registered this task's waker. Wait for that
                // readiness event before trying again, keeping lock futures pinned.
                next = Box::pin(step(runtime));
                Poll::Pending
            }
            result => result,
        })
        .await?;
        match result {
            JobStep::Idle => return Ok(()),
            JobStep::Progress => {
                next = Box::pin(step(runtime));
                tokio::task::yield_now().await;
            }
            JobStep::Waiting => unreachable!(),
        }
    }
}

pub async fn with_timeout<T>(
    context: &AsyncContext,
    timeout: Duration,
    future: impl Future<Output = Result<T, ScriptError>>,
) -> Result<T, ScriptError> {
    let deadline = Instant::now() + timeout;
    let interrupted = Arc::new(AtomicBool::new(false));
    let flag = interrupted.clone();
    context
        .runtime()
        .set_interrupt_handler(Some(Box::new(move || {
            let expired = Instant::now() >= deadline;
            if expired {
                flag.store(true, Ordering::Relaxed);
            }
            expired
        })))
        .await;
    let result = tokio::time::timeout(timeout, future).await;
    // Some interrupted async computations leave their Promise pending. The
    // independent flag is authoritative; do not rely on Promise rejection.
    if interrupted.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err(ScriptError::timeout());
    }
    result.map_err(|_| ScriptError::timeout())?
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use rquickjs::{Function, function::Async};

    use super::super::{create_context, evaluate_module_from_path};

    #[tokio::test(flavor = "current_thread")]
    async fn timed_out_runtime_releases_pending_native_future_captures() {
        struct Capture(Arc<AtomicBool>);

        impl Drop for Capture {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let dropped = Arc::new(AtomicBool::new(false));
        let (runtime, context) = create_context().await;
        context
            .with(|ctx| {
                let dropped = dropped.clone();
                let pending = Function::new(
                    ctx.clone(),
                    Async(move || {
                        let capture = Capture(dropped.clone());
                        async move {
                            let _capture = capture;
                            std::future::pending::<()>().await;
                        }
                    }),
                )?;
                ctx.globals().set("pendingNative", pending)
            })
            .await
            .unwrap();

        let result = evaluate_module_from_path(
            "export default await pendingNative();",
            Path::new("pending.js"),
            &context,
            Duration::from_millis(50),
        )
        .await;
        assert!(result.unwrap_err().to_string().contains("timeout"));
        assert!(!dropped.load(Ordering::SeqCst));
        drop(context);
        drop(runtime);
        assert!(dropped.load(Ordering::SeqCst));
    }
}
