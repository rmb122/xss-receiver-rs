CREATE TABLE users (
	id serial4 NOT NULL,
	username varchar(128) NOT NULL,
	"password" varchar NOT NULL,
	CONSTRAINT users_pk PRIMARY KEY (id),
	CONSTRAINT users_unique UNIQUE (username)
);

CREATE TABLE http_log (
	id serial4 NOT NULL,
	client_ip varchar(45) NOT NULL,
	client_port int4 NOT NULL,
	"method" varchar(255) NOT NULL,
	"path" varchar NOT NULL,
	arg jsonb NOT NULL,
	"header" jsonb NOT NULL,
	body_type int2 NOT NULL,
	body text NOT NULL,
	file jsonb NOT NULL,
	error_log text NULL,
	create_time timestamp DEFAULT now() NOT NULL,
	CONSTRAINT http_log_pk PRIMARY KEY (id)
);
CREATE INDEX http_log_client_ip_idx ON http_log USING btree (client_ip);
CREATE INDEX http_log_create_time_idx ON http_log USING btree (create_time);
CREATE INDEX http_log_method_idx ON http_log USING btree (method);
CREATE INDEX http_log_path_idx ON http_log USING btree (path);
