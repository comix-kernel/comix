# 文档总览（document/）

本目录存放 comix 项目的设计文档、模块说明与写作指南。此 README 面向“文档作者与维护者”，用于解释组织结构、写作约定、如何预览/发布。

## 快速浏览与预览

- 直接阅读：按下述“目录导航”中的链接浏览各子模块文档。
- 使用 mdBook 预览（若需书籍化浏览）：
  1. 安装 mdBook（一次性）
     - 使用 Rust 工具链：`cargo install mdbook`
     - 或从发行包获取，参考 mdBook 官方说明
  2. 在仓库根目录执行（假定 book.toml 位于 document/ 或仓库根）：
     - 预览：`mdbook serve document -n 0.0.0.0 -p 4000`
     - 构建：`mdbook build document`
  3. 浏览器打开预览
     - 终端：`$BROWSER http://127.0.0.1:4000/`
- 无 book.toml 时：你仍可直接阅读 Markdown 文件；如需 mdBook 视图，请新增 book.toml 并确保 SUMMARY.md 正确列出条目。

## 写作与维护约定

- 文件组织
  - 每个子系统一个子目录；跨子系统主题放在更高层次目录（如 kernel 与 mm 的交叉主题）。
  - 子目录首选提供一个该子系统的 README.md 做概览与导航。
- 链接与路径
  - 文档内引用源码时，尽量使用以仓库根为基准的绝对路径提示（便于读者搜索源文件），例如：`os/src/ipc/pipe.rs`
  - 面向 mdBook 的导航请在 SUMMARY.md 中登记；面向贡献者的说明放在各自 README.md。
- 风格与结构
  - 先给结论与关键 API，再给背景与细节；长文档建议提供“导航/目录”与“总结/要点”。
  - 代码片段应最小可读，可附运行/调用路径。
- 校验与工具
  - 风格与链接检查可参考：`document/scripts/style-check.md`、`document/scripts/rewrite_links.md`
  - 提交前自查：新增文档是否需要出现在 SUMMARY.md；是否在对应子目录 README 中被导航到。

## 新增或更新文档的建议流程

1. 在对应子目录内新增 Markdown 文件，或同步更新该子目录 README 的导航。
2. 若需要在 mdBook 中展示，更新 `document/SUMMARY.md`，保持目录结构清晰。
3. 本地预览（如使用 mdBook）：`mdbook serve document -n 0.0.0.0 -p 4000`，浏览器打开：`$BROWSER http://127.0.0.1:4000/`
4. 在 PR 描述中简述新增内容与对应源码位置，便于评审。

## 常见问题

- 为什么我新增的文档在左侧目录看不到？
  - 需要把文档添加到 `document/SUMMARY.md` 中。README 只提供作者指引，不参与 mdBook 的目录生成。
- 链接在 mdBook 中 404？
  - 检查相对路径是否以 `document/` 为根进行组织；必要时使用脚本 `document/scripts/rewrite_links.md` 的建议重写规则。