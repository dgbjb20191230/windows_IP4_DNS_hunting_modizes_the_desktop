@echo off
title 构建应用
color 0A

echo ================================================
echo        Windows网络配置工具构建
echo ================================================
echo.
echo 正在构建前端...
call yarn build

echo.
echo 正在构建Rust应用...
cd src-tauri
cargo build --release
cd ..

echo.
echo 构建完成！
echo 可执行文件位于: src-tauri\target\release\tauri-vue-20250419.exe
echo.
echo 按任意键退出...
pause > nul
