CREATE TABLE users (
	id serial4 NOT NULL,
	username varchar(128) NOT NULL,
	"password" varchar NOT NULL,
	create_time timestamp DEFAULT now() NOT NULL,
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
	extra_info jsonb NOT NULL,
	error_log text NULL,
	create_time timestamp DEFAULT now() NOT NULL,
	CONSTRAINT http_log_pk PRIMARY KEY (id)
);
CREATE INDEX http_log_client_ip_idx ON http_log USING btree (client_ip);
CREATE INDEX http_log_create_time_idx ON http_log USING btree (create_time);
CREATE INDEX http_log_method_idx ON http_log USING btree (method);
CREATE INDEX http_log_path_idx ON http_log USING btree (path);

CREATE TABLE system_log (
	id serial4 NOT NULL,
	log varchar NOT NULL,
	create_time timestamp DEFAULT now() NOT NULL,
	CONSTRAINT system_log_pk PRIMARY KEY (id)
);
CREATE INDEX system_log_create_time_idx ON system_log USING btree (create_time);

CREATE TABLE route (
	id serial4 NOT NULL,
	kind int2 NOT NULL,
	pattern varchar(1024) NOT NULL,
	"timeout" int4 NOT NULL,
	"catalog" varchar(1024) NOT NULL,
	"handler" varchar NOT NULL,
	write_log bool NOT NULL,
	"comment" text NOT NULL,
	create_time timestamp NOT NULL,
	CONSTRAINT route_pk PRIMARY KEY (id)
);
CREATE INDEX route_create_time_idx ON route (create_time);
