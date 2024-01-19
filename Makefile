# Makefile
TARGET_WINDOWS = x86_64-pc-windows-gnu
FEATURE ?= multithreaded

.PHONY: check build check-windows check-linux build-windows build-linux

check: check-windows check-linux

build: build-windows build-linux

all: check build

check-windows:
ifeq ($(FEATURE), singlethreaded)
	cargo check --no-default-features --target=$(TARGET_WINDOWS)

else ifeq ($(FEATURE), multithreaded)
	cargo check --target=$(TARGET_WINDOWS)
else
	@echo "invalid feature: $(FEATURE)"
endif

check-linux:
ifeq ($(FEATURE), singlethreaded)
	cargo check --no-default-features

else ifeq ($(FEATURE), multithreaded)
	cargo check
else
	@echo "invalid feature: $(FEATURE)"
endif


build-windows:
ifeq ($(FEATURE), singlethreaded)
	cargo build --no-default-features --target=$(TARGET_WINDOWS)
else ifeq ($(FEATURE), multithreaded)
	cargo build --target=$(TARGET_WINDOWS)
else
	@echo "invalid feature: $(FEATURE)"
endif

build-linux:
ifeq ($(FEATURE), singlethreaded)
	cargo build --no-default-features
else ifeq ($(FEATURE), multithreaded)
	cargo build
else
	@echo "invalid feature: $(FEATURE)"
endif
