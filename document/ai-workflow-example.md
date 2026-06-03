# AI 工作流示例：getcwd syscall

本文档展示一个任务如何沿工具无关 workflow 推进。示例不代表真实任务状态，真实任务应放在 `.ai-workflow/tasks/<task-name>/`。

## 1. 创建任务

```bash
make workflow-new TASK=getcwd-syscall
```

生成目录：

```text
.ai-workflow/tasks/getcwd-syscall/
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

## 2. 需求阶段

AI 根据口头描述生成 `01-requirement.md`，人工审阅后批准：

```bash
make workflow-approve TASK=getcwd-syscall STAGE=requirement BY=alice NOTE="目标和非目标清楚"
```

如果不通过：

```bash
make workflow-reject TASK=getcwd-syscall STAGE=requirement \
  REJECT_TYPE="需求理解错误" \
  REASON="目标过宽，没有说明是否兼容 Linux ABI" \
  MUST_FIX="补充 ABI 兼容范围和非目标" \
  ROLLBACK=requirement
```

## 3. 规格阶段

AI 基于已批准需求生成 `02-spec.md`，重点补齐：

- syscall 参数定义
- 返回值和 errno
- null pointer、size 为 0、buffer 过小等边界情况
- 测试矩阵

批准：

```bash
make workflow-approve TASK=getcwd-syscall STAGE=spec BY=alice
```

驳回：

```bash
make workflow-reject TASK=getcwd-syscall STAGE=spec \
  REJECT_TYPE="规格不完整" \
  REASON="没有定义 buffer 过小时的 errno" \
  MUST_FIX="补充错误码和对应测试矩阵" \
  ROLLBACK=spec
```

## 4. 实现计划阶段

AI 生成 `03-implementation-plan.md`，说明涉及模块、修改文件、调用链、锁和测试策略。

批准：

```bash
make workflow-approve TASK=getcwd-syscall STAGE=implementation_plan BY=alice
```

## 5. 实现结果阶段

AI 执行代码修改，并在 `04-implementation-result.md` 记录：

- 改了哪些文件
- 实现了哪些行为
- 自检结果
- 已知风险

批准进入 QA：

```bash
make workflow-approve TASK=getcwd-syscall STAGE=impl_loop BY=alice
```

## 6. QA 阶段

运行验证并写入 QA 报告：

```bash
make workflow-verify TASK=getcwd-syscall
```

人工根据 `05-qa-report.md` 判断是否通过：

```bash
make workflow-approve TASK=getcwd-syscall STAGE=qa BY=alice
```

## 7. AI Review 阶段

AI 在 `06-ai-review.md` 中按代码审查方式列 findings，人工确认无阻塞问题后批准：

```bash
make workflow-approve TASK=getcwd-syscall STAGE=ai_review BY=alice
```

## 8. 最终批准与提交前检查

补齐 `07-final-approval.md`、`08-pr-description.md` 和 `commit-message.txt` 后：

```bash
make workflow-approve TASK=getcwd-syscall STAGE=final_approval BY=alice
make workflow-submit-check TASK=getcwd-syscall
```

`workflow-submit-check` 通过后，该任务才进入提交或 PR 阶段。
