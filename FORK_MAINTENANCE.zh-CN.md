# 本地 fork 维护记录

本文记录本机维护 `tingxueren/herdr` fork 时的仓库、构建和安装约定。它用于本地 fork，不代表上游 Herdr 的标准发布流程。

## 当前仓库布局

本机 checkout：

```text
/Users/zhangdongsheng/Work/Git/herdr
```

远端约定：

| remote | 地址 | 用途 |
| --- | --- | --- |
| `origin` | `git@github.com:herdrdev/herdr.git` | 只读获取上游更新 |
| `fork` | `git@github.com:tingxueren/herdr.git` | 本地修复分支和自维护版本 |

当前使用的分支：

```text
fix/fragmented-sgr-mouse
```

该分支跟踪 `fork/fix/fragmented-sgr-mouse`，当前修复提交为 `993020c9`（`fix: preserve fragmented orphaned mouse tails`）。后续修复继续在 fork 分支上提交和发布，不默认向 `origin` 推送或创建上游 PR。

本机 M3 是 **pull-only**：只允许 fetch、pull 和本地验证，禁止从这台机器向任何远端 push。唯一允许 push 的机器是 `dorm-dev`。因此，代码需要发布到 fork 时，应在 dorm-dev 的 checkout 中完成 commit 和 push；M3 只负责拉取已发布的 fork 分支。

M3 的本地 `.git/config` 已将 `origin` 和 `fork` 的 push URL 都设为 `DISABLED`，默认 push 目标也设为 `DISABLED`，并把常见的 GitHub push URL 重写到 `DISABLED`；`.git/hooks/pre-push` 还会提示必须到 dorm-dev 发布。这个 hook 和配置只存在于 M3 的本地 checkout，不要复制到 dorm-dev，否则会把唯一的发布机器也锁死。

M3 上的常用同步命令：

```bash
cd /Users/zhangdongsheng/Work/Git/herdr
git fetch origin --prune
git fetch fork --prune
git switch fix/fragmented-sgr-mouse
git pull --ff-only fork fix/fragmented-sgr-mouse
git status --short --branch
```

在 dorm-dev 上发布 fork 分支：

```bash
ssh dorm-dev
cd /home/agent/Work/Git/herdr
git fetch fork --prune
git switch fix/fragmented-sgr-mouse
git pull --ff-only fork fix/fragmented-sgr-mouse
# 在这里提交、验证，然后只推送到 fork
git push fork fix/fragmented-sgr-mouse
```

dorm-dev 的 checkout 位于 `/home/agent/Work/Git/herdr`，其默认 push 目标已经配置为 `fork`。在 dorm-dev 上不要使用 `origin` 发布；`origin` 只用于获取上游。

发布完成后，回到 M3 同步：

```bash
cd /Users/zhangdongsheng/Work/Git/herdr
git fetch fork --prune
git pull --ff-only fork fix/fragmented-sgr-mouse
```

## 跨机器传文件

M3 和 dorm-dev 之间传源码、文档或补丁时，统一使用 `rsync -avP`。不要使用 `scp`，也不要把整个 checkout 或 `.git` 目录复制过去。

例如把单个文档送到 dorm-dev：

```bash
rsync -avP \
  /Users/zhangdongsheng/Work/Git/herdr/FORK_MAINTENANCE.zh-CN.md \
  dorm-dev:/home/agent/Work/Git/herdr/FORK_MAINTENANCE.zh-CN.md
```

把 dorm-dev 上已经提交的源码或文档同步回 M3：

```bash
rsync -avP \
  dorm-dev:/home/agent/Work/Git/herdr/FORK_MAINTENANCE.zh-CN.md \
  /Users/zhangdongsheng/Work/Git/herdr/FORK_MAINTENANCE.zh-CN.md
```

传输时只选择明确需要的文件或目录；不要同步 `.git/`、`.git/config`、`.git/hooks/`、`target/`、Zig 缓存或个人配置。M3 的 pull-only 设置和 hook 必须留在 M3，不能覆盖 dorm-dev 的发布配置。

如果要开始新的修复，基于当前 fork 分支创建本地分支：

```bash
git switch -c fix/<short-name> fork/fix/fragmented-sgr-mouse
```

`origin` 保留为上游只读 remote；需要同步上游时先 fetch，再在本地评估合并或 rebase。不要把 `origin` 当成自维护版本的推送目标。

## Zig 构建环境

Herdr 的 `crates/ghostty-vt/build.rs` 会构建 vendored `libghostty-vt`，要求 **Zig 0.16.0**。Homebrew 当前的 Zig 版本可能更高，不能直接替代 0.16.0。

本机约定路径：

```text
ZIG=/Users/zhangdongsheng/.local/share/herdr/zig-0.16.0/zig
LIBGHOSTTY_VT_ZIG_SYSTEM_DIR=/Users/zhangdongsheng/herdr-zig-system
```

每次从新 shell 编译或测试前设置：

```bash
export ZIG="$HOME/.local/share/herdr/zig-0.16.0/zig"
export LIBGHOSTTY_VT_ZIG_SYSTEM_DIR="$HOME/herdr-zig-system"
"$ZIG" version
```

应该输出 `0.16.0`。如果忘记设置 `ZIG`，Cargo 会在 build script 阶段直接报：

```text
zig executable not found
```

这表示构建环境不完整，不表示 Rust 源码有编译错误。

Zig 的包下载器也可能不会按预期继承终端代理。公司网络下不要把“shell 中配置了代理”当成依赖已经可用；优先复用 `LIBGHOSTTY_VT_ZIG_SYSTEM_DIR` 中的缓存。这个目录在仓库外维护，不要提交到 Git，也不要为了补依赖修改 vendored 源码。

## 推荐验证顺序

先运行受影响模块的快速回归测试：

```bash
cd /Users/zhangdongsheng/Work/Git/herdr
ZIG="$HOME/.local/share/herdr/zig-0.16.0/zig" \
LIBGHOSTTY_VT_ZIG_SYSTEM_DIR="$HOME/herdr-zig-system" \
cargo test --bin herdr raw_input
```

当前分片 SGR 鼠标修复的验证结果是 `124 passed; 0 failed`。

项目约定的完整检查使用 `just`：

```bash
ZIG="$HOME/.local/share/herdr/zig-0.16.0/zig" \
LIBGHOSTTY_VT_ZIG_SYSTEM_DIR="$HOME/herdr-zig-system" \
just check
```

日常修改完成后，自用 release 构建固定使用：

```bash
ZIG="$HOME/.local/share/herdr/zig-0.16.0/zig" \
LIBGHOSTTY_VT_ZIG_SYSTEM_DIR="$HOME/herdr-zig-system" \
just build-local
```

上面的 Zig 路径是 M3 的本机约定；其他节点使用各自的 Zig 0.16.0 和缓存路径。
提前按上一节导出这两个环境变量后，只需要运行 `just build-local`。

未安装 `just` 时，导出 Zig 环境变量后可直接运行同一个构建脚本，无需额外安装工具：

```bash
bash scripts/build_local.sh
```

`just build-local` 调用这个脚本，仅用于 macOS/Linux 的 fork 日常构建，实际执行：

```bash
CARGO_INCREMENTAL=1 cargo build --release --locked --timings
```

它保留 release 默认优化级别，为本次构建启用 Rust 增量编译，并把耗时报告保存到
`target/cargo-timings/`（设置 `CARGO_TARGET_DIR` 时位于对应目录下）。产物仍是
`target/release/herdr`，与本机已有安装链接一致。`just build` 保留原有行为，
需要按默认 release 设置验证时使用；CI 和发布构建不通过这个本地入口。

首次启用增量编译需要建立缓存，后续以相同参数修改代码再构建时才能评估提速效果。
保留 `target/` 和 Zig 缓存，不要在日常更新前运行 `cargo clean`；缓存会增加磁盘占用。
修改工具链、依赖或构建参数可能触发重编译。无源码变化的再次构建只能验证缓存复用，
不能代表修改源码后的编译耗时。

连续开发时，可以先用 `cargo check --locked` 检查类型和语法，再运行受影响的测试
（例如 `just test-one raw_input`），需要更新日常版本时再运行 `just build-local`。
构建入口不执行测试，提交前仍按项目约定完成验证。

release 优化构建可能明显慢于 debug 测试。构建开始后应先看 `ps` 确认仍有 `rustc` 或 `zig` 活动，不要把长时间没有终端输出误判成死锁。

## 本机安装方式

本地自维护版本安装到：

```text
$HOME/.local/bin/herdr
```

macOS 上不要把 ad-hoc 签名的 release Mach-O 直接复制到新的路径；在本机验证过，复制后的文件可能在 `dyld` 阶段卡住，而原始构建路径可以正常启动。使用符号链接：

```bash
mkdir -p "$HOME/.local/bin"
if [ -e "$HOME/.local/bin/herdr" ] || [ -L "$HOME/.local/bin/herdr" ]; then
  mv "$HOME/.local/bin/herdr" "$HOME/.local/bin/herdr.binary-copy-$(date +%Y%m%d-%H%M%S)"
fi
ln -s "$HOME/Work/Git/herdr/target/release/herdr" "$HOME/.local/bin/herdr"
```

验证 PATH 和版本：

```bash
command -v herdr
which -a herdr
herdr --version
```

应优先命中 `$HOME/.local/bin/herdr`。如果仍然命中 `/opt/homebrew/bin/herdr`，先调整 PATH，再重启本机 Herdr 客户端。Homebrew 版本可以保留作为回退，但不要让它覆盖自维护版本。

本机这次已经卸载 Homebrew Herdr，`$HOME/.local/bin/herdr` 是指向源码 checkout
中 `target/release/herdr` 的符号链接。Pairfob 的 launchd 配置也必须把 `HERDR_BIN`
指向这个入口；只替换 PATH 不会改变 Pairfob 已保存的绝对路径。修改后重新加载
`com.pairfob.pairfob.plist`，再检查 Pairfob 子进程实际加载的 Herdr 路径。

旧的 Herdr TUI 进程可能仍然加载卸载前的 Homebrew 二进制。不要为了切换版本直接
杀掉主 TUI 或其 server；先启动自维护版本并完成 live handoff，待旧窗口自然退出后，
后续从 `$HOME/.local/bin/herdr` 启动即可。

## Radar 工作区空白恢复

如果 Herdr server 重启后侧栏仍显示 `Local`，但工作区下只有一个空白高亮行，先不要
删除 session 或重建 workspace。这个症状通常表示 session 和 pane 仍在，Radar 没有把
`$space_label`、`$space_logo_*` 等 metadata token 重新写回 Herdr；`space_rows()` 会为
工作区保留高度，却因为 token 缺失而画出空白行。

历史记录在 `~/.config/herdr/maintenance-backups/local-before-handoff.json`，其中保留了
同一类“local 多会话未展示”现场。先用 CLI 确认数据仍在：

```bash
export HERDR_ENV=1
$HOME/.local/bin/herdr workspace list
$HOME/.local/bin/herdr agent list
```

如果 workspace/agent 列表正常但 token 缺失，重新执行当前 Radar 安装目录的三步启动链：

```bash
radar_root="$HOME/.config/herdr/plugins/github-installations/hhdebb.herdr-radar-afabda667cbe/16742-0/checkout"
export HERDR_BIN_PATH="$HOME/.local/bin/herdr"
node "$radar_root/bin/agent-icons.js"
node "$radar_root/bin/agent-state.js"
node "$radar_root/bin/agent-view.js" --reapply
```

恢复后检查 `agent-state.js` 进程、`~/.local/state/herdr/plugins/hhdebb.herdr-radar/animator.err`
以及 Herdr server 日志中的 `workspace.report_metadata`。`herdr workspace list` 应重新出现
`space_label` 和相应的 `space_logo_*` token。这个恢复操作属于本机 Radar 运行时维护，
不需要修改 Herdr 源码，也不需要在其他节点重建会话；只有同样出现 token 丢失的节点才需要
执行对应节点的恢复命令。

## 哪些机器需要更新

这次修复位于 `src/raw_input.rs`，解决的是本地 TUI 输入分帧问题。运行自维护 Herdr TUI 的节点需要使用 fork 构建；仅作为远端 server、SSH 端点或 Pairfob relay 的机器不因为这个客户端修复而自动需要更换二进制。

如果某台机器同时运行 Herdr 客户端和 server，则只要该机器的交互式客户端需要这个修复，就在那台机器上安装 fork 构建。远端会话、Codex app-server 和 Pairfob relay 的状态不通过这次客户端更新迁移。

## 变更和推送纪律

- 修改前先检查 `git status`，保留已有未提交改动。
- 提交前至少运行 `git diff --check` 和受影响测试。
- M3 只 pull，不 push；不要在 M3 使用 `git push`、GitHub CLI push 或直接调用远端 URL。
- 只有 dorm-dev 可以执行 `git push fork <branch>`，并且目标只能是 `tingxueren/herdr` fork。
- M3 的 pull-only hook 是本机配置，不要同步到 dorm-dev；跨机同步只传源码和文档，不传 `.git/`。
- 不使用 `git reset --hard`、`git checkout --` 覆盖工作树。
- 不把本地 Zig 缓存、编译产物或个人路径配置提交进仓库。
- 上游是否接受修复另行评估；本地 fork 可以独立维护和安装。
