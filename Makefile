# Makefile for omni

ifeq ($(OS),Windows_NT)
    INSTALL_DIR = $(USERPROFILE)\.omni\bin
    MKDIR = mkdir "$(INSTALL_DIR)" 2>nul || ver >nul
    COPY = copy /Y target\release\omni.exe "$(INSTALL_DIR)\omni.exe"
    PKILL = taskkill /F /IM omni.exe /T 2>nul || ver >nul
    ADD_PATH = powershell -Command "$$currentPath = [Environment]::GetEnvironmentVariable('PATH', 'User'); if ($$currentPath -notmatch '\.omni\\bin') { [Environment]::SetEnvironmentVariable('PATH', $$currentPath + ';$(INSTALL_DIR)', 'User'); Write-Host 'Added $(INSTALL_DIR) to PATH (Please restart terminal)' }"
else
    INSTALL_DIR = $(HOME)/.omni/bin
    MKDIR = mkdir -p "$(INSTALL_DIR)"
    COPY = cp target/release/omni "$(INSTALL_DIR)/omni"
    PKILL = pkill -f omni || true
    ADD_PATH = if ! grep -q '$(INSTALL_DIR)' $(HOME)/.bashrc 2>/dev/null && ! grep -q '$(INSTALL_DIR)' $(HOME)/.zshrc 2>/dev/null; then echo 'export PATH="$$PATH:$(INSTALL_DIR)"' >> $(HOME)/.bashrc; echo 'Added $(INSTALL_DIR) to PATH in .bashrc (Please run source ~/.bashrc)'; fi
endif

.PHONY: all release install clean

all: release install

release:
	$(PKILL)
	cargo build --release

install: release
	$(MKDIR)
	$(COPY)
	@echo "Installed omni to $(INSTALL_DIR)"

first-install: install
	@$(ADD_PATH)

clean:
	cargo clean
