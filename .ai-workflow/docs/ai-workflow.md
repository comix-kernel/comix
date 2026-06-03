# AI 工作流设计

本文档整理 Comix 项目的 AI 辅助开发工作流，用于把口头需求逐步转化为可审查、可实现、可验证、可提交的工程变更。

## 目标

AI workflow 不是让 AI 直接替代开发者写代码，而是把开发过程拆成一组可重复、可审批、可回退的阶段，让 AI 在每个阶段承担合适的工作：

- 将口头描述整理为需求文档。
- 将需求文档细化为规格文档。
- 将规格文档转化为实现计划。
- 在受控循环中修改代码、构建、测试、修复。
- 辅助 QA、代码审查和提交说明生成。
- 在人工审批不通过时，根据结构化反馈回退到正确阶段。

## 主线流程

推荐主线如下：

```text
口头描述
-> AI 澄清问题
-> AI 需求文档
-> 人工批准
-> AI 规格文档
-> 人工批准
-> AI 实现计划
-> 人工确认
-> AI impl loop
-> QA
-> AI Review
-> 人工批准
-> 提交
```

该流程是工具无关的工程约定。任何 AI agent、脚本或人工协作者都可以参与，但不得依赖某个特定工具的私有 workflow 实现来跳过阶段审批。

其中：

- 需求文档回答“要做什么”和“为什么做”。
- 规格文档回答“系统应该如何表现”。
- 实现计划回答“代码应该如何修改”。
- impl loop 负责实际实现、构建、测试和迭代修复。
- QA 验证行为是否符合需求和规格。
- AI Review 检查代码风险、边界情况和可维护性。
- 人工审批负责最终判断需求、风险和结果是否可接受。

## 仓库落地结构

本仓库使用以下目录承载主流程：

```text
.ai-workflow/
  Makefile.include
  README.md
  AGENTS.md
  commands/
    workflow-*.md
  docs/
    README.md
    ai-workflow.md
    ai-workflow-example.md
  scripts/
    workflow_*.py
  skill/
    SKILL.md
  skills/
    ai-workflow/
      SKILL.md
  templates/
    requirement.md
    spec.md
    implementation-plan.md
    implementation-result.md
    qa-report.md
    ai-review.md
    final-approval.md
    pr-description.md
    commit-message.txt
    rejection.md
    status.json
  tasks/
    <task-name>/
      01-requirement.md
      02-spec.md
      03-implementation-plan.md
      04-implementation-result.md
      05-qa-report.md
      06-ai-review.md
      07-final-approval.md
      08-pr-description.md
      commit-message.txt
      rejection.md
      status.json
```

使用方式：

```bash
make workflow-new TASK=getcwd-syscall
make workflow-new TASK=getcwd-syscall WORKFLOW_NEW_FLAGS=--dry-run
make workflow-list
make workflow-next TASK=getcwd-syscall
make workflow-check TASK=getcwd-syscall
make workflow-approve TASK=getcwd-syscall STAGE=requirement BY=<name>
make workflow-reject TASK=getcwd-syscall STAGE=spec REASON="驳回原因"
make workflow-verify TASK=getcwd-syscall
make workflow-submit-check TASK=getcwd-syscall
make workflow-template-check
```

约定：

- `workflow-new` 从模板创建一个任务目录。
- `workflow-list` 列出所有任务及当前阶段。
- `workflow-next` 输出指定任务的下一步。
- `workflow-check` 读取各阶段 Markdown 中的 `状态：` 或 `结论：` 字段并输出当前审批状态。
- `workflow-approve` 将指定阶段标记为批准，并推进到下一个未完成阶段。
- `workflow-reject` 将指定阶段标记为驳回，写入驳回记录，并根据驳回类型或人工指定回退阶段更新状态。
- `workflow-verify` 运行主流程推荐的本地验证命令；传入 `TASK` 时会把命令、退出码和输出摘要追加到 QA 报告。
- `workflow-submit-check` 检查所有阶段是否批准、QA 是否记录验证、PR 描述和 commit message 是否仍有占位内容。
- `workflow-template-check` 检查 workflow 模板自身是否完整。
- 审批动作由人工直接修改任务目录中的阶段文档完成。
- AI 可以生成、修改和解析这些文档，但不能自动代表人工批准。

迁移到其他仓库时，复制整个 `.ai-workflow/` 目录，并在目标仓库 `Makefile` 中加入：

```make
include .ai-workflow/Makefile.include
```

## AI 工具命令与 Skill

该 workflow 主要面向 Claude Code、Codex 和其他 AI coding agent。各阶段除 Makefile 入口外，还提供 AI 可读取的命令提示词：

```text
.ai-workflow/commands/workflow-start.md
.ai-workflow/commands/workflow-requirement.md
.ai-workflow/commands/workflow-spec.md
.ai-workflow/commands/workflow-plan.md
.ai-workflow/commands/workflow-implement.md
.ai-workflow/commands/workflow-qa.md
.ai-workflow/commands/workflow-review.md
.ai-workflow/commands/workflow-finalize.md
.ai-workflow/commands/workflow-rejection.md
```

使用方式：

- Claude Code：可将这些文件复制或链接为项目 slash commands，也可以让 Claude 直接读取对应命令文件执行。
- Codex：可通过 `.ai-workflow/skill/SKILL.md` 或 `.ai-workflow/skills/ai-workflow/SKILL.md` 作为 skill/指令入口。
- 通用 agent：先读取 `.ai-workflow/AGENTS.md`，再读取当前阶段对应的 command 文件。

本地安装：

```bash
make workflow-install-ai-tools
```

该命令会复制：

```text
.ai-workflow/commands/*.md -> .claude/commands/
.ai-workflow/skills/ai-workflow/ -> .codex/skills/ai-workflow/
```

硬规则：

- AI 不得自动批准阶段。
- AI 不得跳过人工 gate。
- AI 只能在当前未完成阶段工作。
- 代码实现前必须确认需求、规格和实现计划均已批准。

常用阶段名：

```text
requirement
spec
implementation_plan
impl_loop
qa
ai_review
final_approval
```

审批示例：

```bash
make workflow-approve TASK=getcwd-syscall STAGE=requirement BY=alice NOTE="需求边界清楚"
```

驳回示例：

```bash
make workflow-reject TASK=getcwd-syscall STAGE=spec \
  REJECT_TYPE="规格不完整" \
  REASON="没有定义 null buffer 和 buffer 过小时的 errno" \
  MUST_FIX="补充边界行为和测试矩阵" \
  ROLLBACK=spec
```

驳回时，如果 `ROLLBACK` 指向更早阶段，脚本会将该回退阶段重新标记为待定，防止状态机继续向后推进。

## 阶段说明

### 口头描述与 AI 澄清

输入通常来自开发者的自然语言描述，例如“实现一个基础 getcwd syscall”或“修复 execve 后用户栈异常”。

AI 在进入文档阶段前，应先识别歧义和边界问题，必要时提出澄清问题。

澄清重点包括：

- 目标功能是什么。
- 不做什么。
- 目标平台或架构范围。
- 是否要求 Linux ABI 兼容。
- 是否需要用户态测试。
- 是否允许改动公共接口或数据结构。

### 需求文档

需求文档用于定义任务边界。

建议包含：

```text
背景
目标
非目标
使用场景
功能边界
验收标准
风险与约束
```

需求文档示例结构：

```markdown
# 需求文档：基础 getcwd syscall

## 背景

当前用户态程序需要获取当前工作目录，但内核尚未提供完整 getcwd 支持。

## 目标

- 用户态程序可以通过 getcwd 获取当前工作目录。
- 行为与 Linux ABI 的基础语义兼容。

## 非目标

- 不实现 mount namespace。
- 不实现 chroot 语义。

## 验收标准

- 正常 buffer 返回当前路径。
- buffer 为空指针时返回错误。
- buffer 过小时返回错误。
```

### 规格文档

规格文档用于定义系统行为，尤其适合 syscall、VFS、进程、内存和设备驱动等模块。

建议包含：

```text
接口定义
输入参数
输出结果
错误码
状态变化
边界情况
与 Linux ABI 的差异
测试矩阵
```

规格文档必须尽量覆盖边界行为，避免实现阶段靠猜测补全。

### 实现计划

实现计划用于把规格落到代码修改路径。

建议包含：

```text
涉及模块
需要新增或修改的文件
核心数据结构
调用链变化
锁与并发影响
unsafe 边界
测试策略
验证命令
回滚方式
```

对 Comix 项目，常见验证命令包括：

```bash
make build
cd os && cargo fmt
cd os && cargo clippy
cd os && make test
```

实际命令应根据任务风险和模块范围调整。

### Impl Loop

impl loop 是 AI 实现阶段的受控循环：

```text
AI 修改代码
-> 格式化
-> 构建
-> 测试
-> 收集失败
-> 分析原因
-> 修复
-> 重新验证
```

循环退出条件：

- 构建与必要测试通过。
- 发现规格或方案问题，需要回退到前一阶段。
- 遇到无法自行判断的风险，需要人工介入。

### QA

QA 关注“行为是否符合需求和规格”。

检查重点：

- 验收标准是否全部满足。
- 用户态程序是否能复现目标行为。
- 错误码和边界行为是否符合规格。
- QEMU 运行日志是否存在 panic、trap 或异常退出。
- 修改是否影响已有功能。

### AI Review

AI Review 采用代码审查视角，优先检查风险，而不是只总结改动。

检查重点：

- 是否破坏 Linux ABI 兼容性。
- 用户指针和用户 buffer 校验是否完整。
- 是否可能 panic。
- 是否存在锁顺序、死锁或资源释放风险。
- 是否引入不必要抽象。
- 是否遗漏关键测试。
- 修改范围是否超出需求。

## 审批不通过处理

人工审批不通过时，不应直接让 AI 继续改代码，而应先记录结构化反馈，再由 AI 判断应该回退到哪个阶段。

通用流程：

```text
人工审批不通过
-> 人类填写驳回模板
-> AI 解析驳回记录
-> AI 判断回退阶段
-> AI 执行对应阶段任务
-> 重新提交审批
```

回退规则：

```text
需求理解错误
-> 回到需求澄清或需求文档

规格不完整
-> 回到规格文档

方案不合理或范围失控
-> 回到实现计划

实现错误或测试失败
-> 回到 impl loop

代码风险
-> 轻微问题回到 impl loop
-> 架构风险回到实现计划

最终提交不通过
-> 根据驳回原因回退到对应阶段
```

如果人工填写了明确的“期望回退阶段”，AI 默认服从。只有当填写内容与驳回原因明显矛盾时，AI 应先说明矛盾并请求确认。

## 审批驳回模板

人工审批不通过时，建议填写以下模板：

```markdown
# 审批驳回记录

## 驳回阶段

需求文档 / 规格文档 / 实现计划 / 实现结果 / QA / AI Review / 最终提交

## 驳回类型

需求理解错误 / 规格不完整 / 方案不合理 / 实现错误 / 测试失败 / 代码风险 / 范围失控 / 其他

## 驳回原因

请描述不通过的具体原因。

## 必须修改

列出必须修改的内容。

## 可选建议

列出可选建议，没有可留空。

## 是否允许 AI 继续

允许 / 不允许 / 需要先提问澄清

## 期望回退阶段

需求澄清 / 需求文档 / 规格文档 / 实现计划 / Impl Loop / QA / AI Review / 不确定
```

## AI 驳回解析输出

AI 收到驳回模板后，应输出结构化解析结果：

```markdown
# AI 驳回解析结果

## 判定回退阶段

规格文档

## 判定理由

驳回原因属于边界行为未定义，问题发生在规格层，而不是实现层。

## 下一步动作

重新生成规格文档，补充 null 指针、buffer 过小、错误码和测试矩阵。

## 是否需要人工澄清

否
```

## 示例

人工驳回记录：

```markdown
# 审批驳回记录

## 驳回阶段

规格文档

## 驳回类型

规格不完整

## 驳回原因

没有说明 getcwd 在 buffer 为 null、size 为 0、buffer 太小时的行为。

## 必须修改

补充这些边界情况对应的返回值和 errno。

## 可选建议

参考 Linux getcwd 行为。

## 是否允许 AI 继续

允许

## 期望回退阶段

规格文档
```

AI 解析结果：

```markdown
# AI 驳回解析结果

## 判定回退阶段

规格文档

## 判定理由

问题是 syscall 边界行为没有定义，属于规格缺失。

## 下一步动作

更新 getcwd 规格，补充错误码、边界行为和测试矩阵。

## 是否需要人工澄清

否
```

## 推荐优先落地方向

Comix 当前是教学和实验型内核项目，AI workflow 建议优先服务于以下三类任务：

### Bug 修复 workflow

输入：

- panic 日志
- QEMU 输出
- 失败命令
- 相关复现步骤

输出：

- 原因分析
- 修复补丁
- 回归测试
- 验证结果

### Syscall 开发 workflow

输入：

- syscall 名称
- 目标行为
- ABI 兼容要求

输出：

- 需求文档
- 规格文档
- 实现计划
- 内核实现
- 用户态测试
- 兼容性说明

### 文档同步 workflow

输入：

- 已完成的模块改动
- 相关代码路径

输出：

- 模块设计说明
- 调用链说明
- 已实现和待实现能力
- 调试与测试说明

## 原则

- AI 可以建议回退阶段，但人工可以覆盖 AI 判断。
- 每次审批不通过都必须转化为结构化反馈。
- 不通过不是失败，而是受控回退。
- 规格不清楚时不进入实现。
- 实现计划不清楚时不进入代码修改。
- 测试失败时优先分析失败原因，而不是盲目改动。
- 最终提交前必须确认需求、规格、实现、QA 和 Review 均闭环。
