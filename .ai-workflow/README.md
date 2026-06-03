# AI Workflow Pack

这是一个工具无关的 AI 辅助开发 workflow 包。它把需求、规格、实现、QA、AI Review、审批、驳回和提交前检查集中在 `.ai-workflow/` 下，便于迁移到其他仓库。

## 目录

```text
.ai-workflow/
  Makefile.include
  README.md
  AGENTS.md
  commands/
  docs/
  scripts/
  skill/
  skills/
  templates/
  tasks/
```

## 迁移方式

1. 将整个 `.ai-workflow/` 目录复制到目标仓库根目录。
2. 在目标仓库的 `Makefile` 中加入：

```make
include .ai-workflow/Makefile.include
```

3. 运行模板检查：

```bash
make workflow-template-check
```

4. 创建任务：

```bash
make workflow-new TASK=example-task
```

## 常用命令

```bash
make workflow-help
make workflow-new TASK=<name>
make workflow-list
make workflow-next TASK=<name>
make workflow-check TASK=<name>
make workflow-approve TASK=<name> STAGE=<stage> BY=<name>
make workflow-reject TASK=<name> STAGE=<stage> REASON="..."
make workflow-verify TASK=<name>
make workflow-submit-check TASK=<name>
make workflow-template-check
make workflow-install-ai-tools
```

## AI 工具集成

本包面向 Claude Code、Codex 和其他 AI coding agent。阶段动作已经提供为两种形式：

- `commands/`：每个阶段一个命令提示词，可复制成 Claude Code slash command、Codex prompt command，或直接让 AI 读取执行。
- `skill/SKILL.md`：单文件 skill 入口。
- `skills/ai-workflow/SKILL.md`：标准 skill 目录形式，便于复制到支持 skill 的工具目录。
- `AGENTS.md`：AI agent 必须遵守的硬规则，可复制到项目根目录。

本地安装到常见工具目录：

```bash
make workflow-install-ai-tools
```

等价于：

```bash
python3 .ai-workflow/scripts/workflow_install_ai_tools.py --all
```

当前会复制：

```text
.ai-workflow/commands/*.md -> .claude/commands/
.ai-workflow/skills/ai-workflow/ -> .codex/skills/ai-workflow/
```

人类审批仍然通过 Makefile 命令完成。AI 不应自动批准阶段。

## 文档

- [AI 工作流设计](docs/ai-workflow.md)
- [AI 工作流示例](docs/ai-workflow-example.md)
