# Makefile for omnicommand

ifeq ($(OS),Windows_NT)
    INSTALL_DIR = $(USERPROFILE)\.omnicommand\bin
    MKDIR = if not exist "$(INSTALL_DIR)" mkdir "$(INSTALL_DIR)"
    COPY = copy /Y target\release\omnicommand.exe "$(INSTALL_DIR)\omnicommand.exe"
    PKILL = taskkill /F /IM omnicommand.exe /T 2>nul || ver >nul
else
    INSTALL_DIR = $(HOME)/.omnicommand/bin
    MKDIR = mkdir -p "$(INSTALL_DIR)"
    COPY = cp target/release/omnicommand "$(INSTALL_DIR)/omnicommand"
    PKILL = pkill -f omnicommand || true
endif

.PHONY: all build install clean

all: build

build:
	cargo build --release

install:
	-$(PKILL)
	$(MAKE) build
	$(MKDIR)
	$(COPY)
	@echo "Installed omnicommand to $(INSTALL_DIR)"

clean:
	cargo clean
