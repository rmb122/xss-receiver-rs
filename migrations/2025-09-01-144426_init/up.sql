CREATE TABLE users (
	id serial4 NOT NULL,
	username varchar(128) NOT NULL,
	"password" varchar NOT NULL,
	CONSTRAINT users_pk PRIMARY KEY (id),
	CONSTRAINT users_unique UNIQUE (username)
);