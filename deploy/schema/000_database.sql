-- Dev-only (docker-entrypoint-initdb.d): in-cluster the Helm migration job
-- creates the database itself, with the Replicated engine when ha=true.
CREATE DATABASE IF NOT EXISTS ekokube;
