@echo off
cd /d "%~dp0"
cargo test --test document_test -- --nocapture %*
