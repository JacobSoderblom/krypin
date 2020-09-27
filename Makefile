M = $(shell printf "\033[34;1m▶\033[0m")

CMD = ${CMD}

start:
	go get github.com/githubnemo/CompileDaemon
	CompileDaemon -log-prefix=false -exclude="bindata.go" -build="make CMD=$(CMD) build" -command="./tmp/$(CMD)"

deps: ; $(info $(M) Installing dependencies...)
	go mod download

schema: ; $(info $(M) Embedding schema files into binary...)
	go get -u github.com/jteeuwen/go-bindata/...
	go generate ./pkg/graphql/internal/schema

clean: ; $(info $(M) [TODO] Removing generated files... )
	$(RM) ./pkg/api/schema/bindata.go

build: deps ; $(info $(M) Building project...)
	go build -o ./tmp/$(CMD) ./cmd/$(CMD)/*.go

image: ; $(info $(M) Building application image...)
	podman build -t krypin .

.PHONY: deps schema image
