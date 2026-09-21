use anyhow::{Result, anyhow};

#[derive(Debug)]
pub(super) enum Expression {
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Not(Box<Self>),
    Condition(Condition),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Operator {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Contains,
}

#[derive(Debug)]
pub(super) enum Value {
    String(String),
    Integer(i32),
    Null,
}

#[derive(Debug)]
pub(super) struct Condition {
    pub field: String,
    pub operator: Operator,
    pub value: Value,
    pub position: usize,
}

impl Condition {
    pub fn error(&self, message: impl std::fmt::Display) -> anyhow::Error {
        anyhow!("filter at character {}: {message}", self.position)
    }

    pub fn string(&self) -> Result<&str> {
        match &self.value {
            Value::String(value) => Ok(value),
            _ => Err(self.error(format!("{} requires a quoted string", self.field))),
        }
    }

    pub fn integer(&self) -> Result<i32> {
        match self.value {
            Value::Integer(value) => Ok(value),
            _ => Err(self.error(format!("{} requires an integer", self.field))),
        }
    }
}

pub(super) fn parse(input: &str) -> Result<Option<Expression>> {
    let mut parser = Parser {
        input,
        offset: 0,
        nodes: 0,
    };
    parser.skip_whitespace();
    if parser.remaining().is_empty() {
        return Ok(None);
    }
    let expression = parser.or(0)?;
    parser.skip_whitespace();
    if !parser.remaining().is_empty() {
        return Err(parser.error("unexpected input"));
    }
    Ok(Some(expression))
}

struct Parser<'a> {
    input: &'a str,
    offset: usize,
    nodes: usize,
}

impl Parser<'_> {
    fn remaining(&self) -> &str {
        &self.input[self.offset..]
    }

    fn position(&self) -> usize {
        self.input[..self.offset].chars().count() + 1
    }

    fn error(&self, message: &str) -> anyhow::Error {
        anyhow!("filter at character {}: {message}", self.position())
    }

    fn skip_whitespace(&mut self) {
        self.offset += self.remaining().len() - self.remaining().trim_start().len();
    }

    fn take(&mut self, token: &str) -> bool {
        self.skip_whitespace();
        if self.remaining().starts_with(token) {
            self.offset += token.len();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, token: &str) -> Result<()> {
        if !self.take(token) {
            return Err(self.error(&format!("expected '{token}'")));
        }
        Ok(())
    }

    fn add_node(&mut self) -> Result<()> {
        self.nodes += 1;
        // Bound both parser recursion and the generated database expression tree.
        if self.nodes > 256 {
            return Err(self.error("expression is too complex (maximum 256 nodes)"));
        }
        Ok(())
    }

    fn or(&mut self, depth: usize) -> Result<Expression> {
        let mut expression = self.and(depth)?;
        while self.take("||") {
            self.add_node()?;
            expression = Expression::Or(Box::new(expression), Box::new(self.and(depth)?));
        }
        Ok(expression)
    }

    fn and(&mut self, depth: usize) -> Result<Expression> {
        let mut expression = self.unary(depth)?;
        while self.take("&&") {
            self.add_node()?;
            expression = Expression::And(Box::new(expression), Box::new(self.unary(depth)?));
        }
        Ok(expression)
    }

    fn unary(&mut self, depth: usize) -> Result<Expression> {
        if depth > 64 {
            return Err(self.error("expression nesting exceeds 64 levels"));
        }
        if self.take("!") {
            self.add_node()?;
            return Ok(Expression::Not(Box::new(self.unary(depth + 1)?)));
        }
        if self.take("(") {
            let expression = self.or(depth + 1)?;
            self.expect(")")?;
            return Ok(expression);
        }
        self.add_node()?;
        self.condition().map(Expression::Condition)
    }

    fn identifier(&mut self) -> Result<String> {
        self.skip_whitespace();
        let start = self.offset;
        let mut chars = self.remaining().chars();
        if !chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            return Err(self.error("expected a field name"));
        }
        self.offset += self
            .remaining()
            .bytes()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
            .count();
        Ok(self.input[start..self.offset].to_owned())
    }

    fn condition(&mut self) -> Result<Condition> {
        self.skip_whitespace();
        let position = self.position();
        let mut field = self.identifier()?;
        let operator;
        let value;
        if field == "contains" {
            self.expect("(")?;
            field = self.identifier()?;
            self.expect(",")?;
            value = self.value()?;
            self.expect(")")?;
            operator = Operator::Contains;
        } else {
            operator = [
                ("!=", Operator::NotEqual),
                ("<=", Operator::LessEqual),
                (">=", Operator::GreaterEqual),
                ("=", Operator::Equal),
                ("<", Operator::Less),
                (">", Operator::Greater),
            ]
            .into_iter()
            .find_map(|(token, operator)| self.take(token).then_some(operator))
            .ok_or_else(|| self.error("expected a comparison operator"))?;
            value = self.value()?;
        }
        Ok(Condition {
            field,
            operator,
            value,
            position,
        })
    }

    fn string(&mut self, quote: char) -> Result<String> {
        self.offset += 1;
        // Normalize quotes and apostrophe escapes, then use JSON to decode the rest.
        let mut json = String::from("\"");
        let mut escaped = false;
        for (offset, character) in self.remaining().char_indices() {
            if escaped {
                if character != '\'' {
                    json.push('\\');
                }
                json.push(character);
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == quote {
                json.push('"');
                let value = serde_json::from_str(&json)
                    .map_err(|error| self.error(&format!("invalid string: {error}")))?;
                self.offset += offset + 1;
                return Ok(value);
            } else {
                if character == '"' {
                    json.push('\\');
                }
                json.push(character);
            }
        }
        Err(self.error("unterminated string"))
    }

    fn value(&mut self) -> Result<Value> {
        self.skip_whitespace();
        let start = self.offset;
        if let Some(quote @ ('"' | '\'')) = self.remaining().chars().next() {
            return self.string(quote).map(Value::String);
        }
        if self.take("null") {
            return Ok(Value::Null);
        }
        if self.remaining().starts_with('-') {
            self.offset += 1;
        }
        let digits = self
            .remaining()
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        if digits == 0 {
            return Err(self.error("expected a quoted string, integer or null"));
        }
        self.offset += digits;
        let number = self.input[start..self.offset]
            .parse()
            .map_err(|_| self.error("integer is outside the 32-bit range"))?;
        Ok(Value::Integer(number))
    }
}
