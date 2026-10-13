.PHONY: run build check clean volume-up volume-ls pull-image help

# Runtime container: cek daemon yang hidup dulu (docker -> podman)
RUNTIME ?= $(shell docker info >/dev/null 2>&1 && echo docker || echo podman)

# Konfigurasi default (samakan dengan src/config.rs)
CONTAINER_NAME ?= pgadmin4
IMAGE          ?= docker.io/dpage/pgadmin4:latest
HOST_PORT      ?= 5050
EMAIL          ?= pgadmin4@pgadmin.org
PASSWORD       ?= admin123
VOLUME         ?= pgadmin-data

help:
	@echo "Targets:"
	@echo "  make run          - build & jalankan pgAdmin Manager (GUI)"
	@echo "  make build        - build release"
	@echo "  make check        - cargo check"
	@echo "  make pull-image   - tarik image pgAdmin"
	@echo "  make volume-up    - presetup named volume agar data persist"
	@echo "  make volume-ls    - lihat volume + inspect"
	@echo "  make clean        - hapus target/"
	@echo ""
	@echo "Variabel (bisa di-override): RUNTIME=$(RUNTIME) IMAGE=$(IMAGE) HOST_PORT=$(HOST_PORT)"

run:
	cargo run

build:
	cargo build --release

check:
	cargo check

pull-image:
	$(RUNTIME) pull $(IMAGE)

# Presetup volume: idempoten (aman dijalankan berulang). Data pgAdmin persist di sini.
volume-up:
	$(RUNTIME) volume create $(VOLUME)
	@echo "Volume '$(VOLUME)' siap. Data pgAdmin persist di $(VOLUME) -> /var/lib/pgadmin"

volume-ls:
	$(RUNTIME) volume ls | grep -i pgadmin || true
	$(RUNTIME) volume inspect $(VOLUME) || true

clean:
	cargo clean
