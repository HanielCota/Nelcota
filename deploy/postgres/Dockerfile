# The project's Postgres: the official postgres:17-alpine image plus
# pgBackRest, which archives WAL to S3 for point-in-time recovery
# (`nelcota pitr enable`). Same base image and libc as before, so an
# existing data directory (and its text indexes) is unchanged.
FROM postgres:17-alpine
RUN apk add --no-cache pgbackrest
