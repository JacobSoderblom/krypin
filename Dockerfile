FROM golang:1.14.3-alpine as base
RUN echo -e "http://nl.alpinelinux.org/alpine/v3.5/main\nhttp://nl.alpinelinux.org/alpine/v3.5/community" > /etc/apk/repositories
RUN apk update && apk upgrade && \
apk add --no-cache bash git openssh
RUN apk add --no-cache autoconf automake libtool gettext gettext-dev make g++ texinfo curl

COPY . /app
WORKDIR /app
RUN go mod download
RUN go get github.com/githubnemo/CompileDaemon

ENTRYPOINT CompileDaemon -log-prefix=false -build="go build -o ./tmp/krypin ./cmd/krypin/*.go" -command="./tmp/krypin"