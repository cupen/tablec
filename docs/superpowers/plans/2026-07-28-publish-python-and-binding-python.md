# binding-python 打包修正 + publish-python.yml Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `pip install tablec` 在干净 venv 里直接走通,并在 push tag 时自动把 wheel attach 到 GitHub Release。

**Architecture:**
- 改 `binding-python/pyproject.toml` 把 PyPI 包名从 `tablec-python` 改成 `tablec`,补齐 PEP 621 metadata(description / authors / license / urls / classifiers),并让 maturin 引用 workspace 根的 LICENSE
- 修 `binding-python/README.md` 里的 typo 与包名引用,加一节"Import name vs package name"避免读者疑惑
- 新增 `.github/workflows/publish-python.yml`,在 `push tag v*` 时用 `maturin build --release --strip --compatibility manylinux2014` 出 wheel,attach 到 release.yml 已创建的 GitHub Release(走轮询避免 race)

**Tech Stack:** maturin 1.9+、pyo3 0.27.2、uv 0.x、Python 3.10+、GitHub Actions、swatinem/rust-cache v2、softprops/action-gh-release 不依赖(改用 `gh release upload`)

## Global Constraints

- PyPI 包名 = import 名 = `tablec`(PEP 421 不再让两者分裂)
- 包版本从 `binding-python/pyproject.toml` 的 `[project].version` 读;git tag 与 version 手工保证一致
- 平台:仅 Linux x86_64(manylinux2014)
- 触发:push tag `v*` + `workflow_dispatch`
- 复用 `release.yml` 创建的 GitHub Release,本 workflow 不创建 release
- 不动 `binding-python/src/lib.rs`、Cargo workspace、Cargo.toml、ci.yml、release.yml
- 不发 PyPI(明天再写 publish job)
- 不动 Cargo.toml(abi3 features 冗余是另一个 spec)
- Python ≥ 3.10(abi3 范围)
- maturin `>=1.9.0,<2.0`
- 提交走 `feat/publish-python-spec` 分支(per push-policy:3+ commits 自起分支)

**Pre-existing facts the executor should not re-verify:**

- `Cargo.toml` 中 pyo3 features 同时打开 `abi3-py310..abi3-py313` 是冗余但能编译,本 spec 不动
- `binding-python/Cargo.toml` 的 `name = "binding-python"` 是 crate 名,跟 PyPI 包名无关
- `release.yml` 的 `create release` job 内部 `needs: build`,所以 release 页出现的最早时刻 ≈ build 三个平台完成时间(实测 ~3 min,远小于 10 min 轮询上限)
- `binding-python/` 已经有 `tests/test_python_binding.py` 等三份 pytest,直接 `pytest` 就能跑(`maturin develop` 后 `import tablec` 可用)
- `bd` 已初始化并已创建 `tablec-jbo` / `tablec-bh6` 两条 issue;本 plan 末尾统一 `bd close`

---

## File Structure

**Modified files:**

- `binding-python/pyproject.toml` — `[project].name` 改 `tablec`;新增 description / readme / license / license-files / authors / keywords / classifiers / urls;`[tool.maturin]` 加 `strip = true`
- `binding-python/README.md` — typo 修正 + 包名引用替换 + 新增"Import name vs package name"小节

**Created files:**

- `.github/workflows/publish-python.yml` — 新 workflow(`build-wheel` + `attach-release` 两个 job)

**Untouched (verified):**

- `binding-python/Cargo.toml`、`binding-python/src/lib.rs`、`Cargo.toml`(workspace 根)
- `.github/workflows/ci.yml`、`.github/workflows/release.yml`
- `binding-python/src/`、`binding-python/tests/`、`binding-python/tablec/`

---

## Task 1: 重写 `binding-python/pyproject.toml` — 改名 + 补 metadata

**Files:**
- Modify: `binding-python/pyproject.toml`(整文件重写)
- Test: 跑 `binding-python/tests/test_python_binding.py`(已有,作为回归)

**Interfaces:**
- Consumes: workspace 根的 `/home/bot/workbench/repos/tablec/LICENSE`(MIT,Python 用 `../LICENSE` 引用)
- Produces: 一个能被 `maturin develop` 正常 link、产出 `import tablec` 的可用包,PEP 621 metadata 完整

### Steps

- [ ] **Step 1.1:基线检查 — 当前 pyproject.toml 解析无误 + pytest 通过**

```bash
cd /home/bot/workbench/repos/tablec/binding-python
python3 -c "import tomllib; tomllib.load(open('pyproject.toml','rb')); print('ok')"
```

预期:输出 `ok`。

```bash
cd /home/bot/workbench/repos/tablec/binding-python
uv venv .venv-baseline --python 3.11
source .venv-baseline/bin/activate
uv pip install maturin pytest openpyxl
maturin develop --release
pytest tests/ -v
```

预期:`maturin develop` 成功,pytest 全绿(3 个 test_*.py)。

- [ ] **Step 1.2:用下面整文件重写 `binding-python/pyproject.toml`**

```toml
[build-system]
requires = ["maturin>=1.9.0,<2.0"]
build-backend = "maturin"

[project]
name = "tablec"
version = "0.1.0"
description = "Python bindings for tablec — a Rust-based table compiler for gamedev (Excel/CSV/JSON -> JSON/MessagePack)"
readme = "README.md"
requires-python = ">=3.10"
license = "MIT"
license-files = ["../LICENSE"]
authors = [
    { name = "cupen", email = "xcupen@gmail.com" },
]
keywords = ["tablec", "excel", "gamedev", "table", "compiler", "msgpack"]
classifiers = [
    "Development Status :: 3 - Alpha",
    "Intended Audience :: Developers",
    "License :: OSI Approved :: MIT License",
    "Operating System :: POSIX :: Linux",
    "Operating System :: MacOS",
    "Operating System :: Microsoft :: Windows",
    "Programming Language :: Rust",
    "Programming Language :: Python :: 3",
    "Programming Language :: Python :: 3.10",
    "Programming Language :: Python :: 3.11",
    "Programming Language :: Python :: 3.12",
    "Programming Language :: Python :: 3.13",
    "Programming Language :: Python :: Implementation :: CPython",
    "Programming Language :: Python :: Implementation :: PyPy",
    "Topic :: Software Development :: Compilers",
    "Topic :: Games/Entertainment",
]
dependencies = [
    "openpyxl>=3.1.5",
]

[project.urls]
Homepage = "https://github.com/cupen/tablec"
Repository = "https://github.com/cupen/tablec"
Documentation = "https://github.com/cupen/tablec#readme"
Issues = "https://github.com/cupen/tablec/issues"

[tool.maturin]
features = ["pyo3/extension-module"]
module-name = "tablec._native"
python-source = "."
strip = true

[dependency-groups]
dev = [
    "pytest>=8.4.1",
    "openpyxl>=3.1.5",
]
```

> 说明:`license-files = ["../LICENSE"]` — maturin 1.9+ 把 workspace 根的 `LICENSE` 拷进 wheel 元数据。如果 maturin 报路径错,改成 `["../../LICENSE"]` 或把 LICENSE 复制到 `binding-python/`。

- [ ] **Step 1.3:验证 pyproject.toml 解析无误 + pytest 仍通过**

```bash
cd /home/bot/workbench/repos/tablec/binding-python
python3 -c "import tomllib; d=tomllib.load(open('pyproject.toml','rb')); print(d['project']['name'], d['project']['license'], d['project']['license-files'])"
```

预期:输出形如 `tablec MIT ['../LICENSE']`。

```bash
cd /home/bot/workbench/repos/tablec/binding-python
rm -rf .venv-baseline
uv venv .venv-test --python 3.11
source .venv-test/bin/activate
uv pip install maturin pytest openpyxl
maturin develop --release
pytest tests/ -v
```

预期:pytest 仍全绿;若 `import tablec` 报 `ModuleNotFoundError` 说明 `name` 改动后 maturin 的 `module-name` 不匹配 — 回查 `module-name = "tablec._native"` 是否仍正确。

- [ ] **Step 1.4:Commit**

```bash
cd /home/bot/workbench/repos/tablec
git add binding-python/pyproject.toml
git commit -m "fix(python): rename package to tablec, add full PEP 621 metadata"
```

---

## Task 2: 改 `binding-python/README.md` — 修 typo + 包名 + 新增 import 名称说明

**Files:**
- Modify: `binding-python/README.md`

**Interfaces:**
- Consumes: Task 1 改完的 pyproject.toml
- Produces: README 不再有 `matisin` typo、不再有 `pip install tablec-python` 引用;新增一节明确"装的名字 = import 的名字"

### Steps

- [ ] **Step 2.1:确认 baseline 状态**

```bash
cd /home/bot/workbench/repos/tablec/binding-python
grep -nE 'matisin|tablec-python' README.md
```

预期:至少 2 行(typo + 包名);记录具体行号后续会变。

- [ ] **Step 2.2:用 Edit 工具做下面 4 处替换(行号是当前 README 真实行号,执行时若 README 内容已变,以 grep 实际行号为准)**

**Edit 1:Development section 的 maturin 写法(当前第 15 行 `uv matisin develop`)**

```
- [ ] **Edit 1**:old:
  uv matisin develop
- [ ] **Edit 1**:new:
  uv run maturin develop
```

**Edit 2:`### From wheel (future)` 节的 pip install(当前第 21 行 `pip install tablec-python`)**

```
- [ ] **Edit 2**:old:
  pip install tablec-python
- [ ] **Edit 2**:new:
  pip install tablec
```

**Edit 3:`## Building for Distribution` 节的两行(当前第 118、121 行)**

```
- [ ] **Edit 3**:old:
  uv maturin build --release
- [ ] **Edit 3**:new:
  uv run maturin build --release
```

```
- [ ] **Edit 3b**:old:
  uv maturin build --release --strip --compatibility manylinux
- [ ] **Edit 3b**:new:
  uv run maturin build --release --strip --compatibility manylinux
```

**Edit 4:在 `## License` 节前插入新一节"Import name vs package name"(紧跟现有 `## License` 标题前一行)**

在 README 文件里找到 `## License` 标题,在它前面插入:

```markdown
## Import name vs package name

The package published to PyPI is `tablec`. The Python import name is also `tablec`
(PEP 421 allows the two to differ, but here they match on purpose):

\`\`\`bash
pip install tablec
python -c "import tablec; tablec.check('your.xlsx')"
\`\`\`

If you `pip install` a checkout of this repo, the editable install uses the
project's `name` (`tablec`) — `import tablec` is the canonical import.

```

- [ ] **Step 2.3:验证 — 不再出现 `matisin` 或 `tablec-python` 字符串**

```bash
cd /home/bot/workbench/repos/tablec/binding-python
grep -nE 'matisin|tablec-python' README.md
echo "---"
grep -nE '^\#\# Import name vs package name' README.md
```

预期:第一个 grep 退出码非零(无匹配);第二个 grep 命中那一行(显示新章节标题)。

- [ ] **Step 2.4:验证 — README 的 `## Building for Distribution` 段里 maturin 命令用 `uv run maturin ...`**

```bash
cd /home/bot/workbench/repos/tablec/binding-python
grep -nE 'uv run maturin' README.md
```

预期:至少 3 行(`develop` 一次 + `build --release` 一次 + `build --release --strip --compatibility manylinux` 一次)。

- [ ] **Step 2.5:Commit**

```bash
cd /home/bot/workbench/repos/tablec
git add binding-python/README.md
git commit -m "doc(python): fix maturin typo, package name, add import-name note"
```

---

## Task 3: 新增 `.github/workflows/publish-python.yml`

**Files:**
- Create: `.github/workflows/publish-python.yml`

**Interfaces:**
- Consumes: Task 1 改完的 `binding-python/pyproject.toml`(package name `tablec`,module name `tablec._native`)
- Produces: 一个 push tag `v*` 时构建 Linux manylinux2014 x86_64 wheel,attaches 到 release.yml 创建的 GitHub Release 的 workflow 文件;不创建 release 本身,不推 PyPI

### Steps

- [ ] **Step 3.1:确认 .github/workflows/ 目录存在**

```bash
ls /home/bot/workbench/repos/tablec/.github/workflows/
```

预期:`ci.yml`、`release.yml` 存在。

- [ ] **Step 3.2:用下面整文件创建 `.github/workflows/publish-python.yml`**

```yaml
name: publish-python

on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  build-wheel:
    name: Build wheel (linux x86_64)
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: binding-python -> ../target

      - name: Setup Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - name: Install maturin
        run: pip install "maturin>=1.9.0,<2.0"

      - name: Build wheel
        working-directory: binding-python
        run: maturin build --release --strip --compatibility manylinux2014 --out dist/wheels

      - name: Show built artifacts
        working-directory: binding-python
        run: ls -lh dist/wheels/

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: python-wheels
          path: binding-python/dist/wheels/*.whl
          if-no-files-found: error

  attach-release:
    name: Attach wheel to GitHub Release
    needs: build-wheel
    runs-on: ubuntu-latest
    timeout-minutes: 15
    permissions:
      contents: write
    steps:
      - name: Download wheel artifact
        uses: actions/download-artifact@v4
        with:
          name: python-wheels
          path: dist

      - name: Install gh
        run: type gh || (apt-get update -qq && apt-get install -y -qq gh)

      - name: Resolve tag
        id: tag
        run: |
          if [ "${{ github.event_name }}" = "workflow_dispatch" ]; then
            TAG=$(gh release list --limit 1 --json tagName --jq '.[0].tagName // empty')
            if [ -z "$TAG" ]; then
              echo "::error::workflow_dispatch: no existing release found; push a v* tag first"
              exit 1
            fi
          else
            TAG=${GITHUB_REF#refs/tags/}
          fi
          echo "tag=$TAG" >> "$GITHUB_OUTPUT"
          echo "Resolved tag: $TAG"
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Wait for release.yml to create the release
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAG: ${{ steps.tag.outputs.tag }}
        run: |
          echo "Waiting for release $TAG to be created by release.yml ..."
          for i in $(seq 1 40); do
            if gh release view "$TAG" >/dev/null 2>&1; then
              echo "Release $TAG exists."
              exit 0
            fi
            echo "  attempt $i/40 — not yet, sleeping 15s"
            sleep 15
          done
          echo "::error::Release $TAG not found after 10 minutes"
          exit 1

      - name: Upload wheel to release
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAG: ${{ steps.tag.outputs.tag }}
        run: |
          set -euo pipefail
          shopt -s nullglob
          wheels=(dist/*.whl)
          if [ ${#wheels[@]} -eq 0 ]; then
            echo "::error::No wheels downloaded"
            exit 1
          fi
          echo "Uploading: ${wheels[*]}"
          gh release upload "$TAG" "${wheels[@]}" --clobber
```

- [ ] **Step 3.3:验证 YAML 语法正确(用 Python `yaml.safe_load`,不依赖 actionlint)**

```bash
python3 -c "import yaml,sys; d=yaml.safe_load(open('/home/bot/workbench/repos/tablec/.github/workflows/publish-python.yml')); print('jobs:', list(d['jobs'].keys())); print('triggers:', list(d[True].keys()) if True in d else list(d.get('on',{}).keys()))"
```

预期:输出形如:
```
jobs: ['build-wheel', 'attach-release']
triggers: ['push', 'workflow_dispatch']
```

如果 `pyyaml` 不在系统 Python,临时装:
```bash
python3 -m pip install --user pyyaml 2>&1 | tail -2
python3 -c "import yaml; print('ok')"
```

- [ ] **Step 3.4:验证 workflow 中关键决策点都符合 spec**

```bash
cd /home/bot/workbench/repos/tablec
grep -nE 'tags:|manylinux2014|--compatibility|attach-release|gh release upload|wait|Wait' .github/workflows/publish-python.yml | head -20
```

预期:命中行包括 `tags:` 下的 `- 'v*'`、`--compatibility manylinux2014`、`gh release upload`、`Wait for release.yml`、`40` 轮询上限等。

- [ ] **Step 3.5:Commit**

```bash
cd /home/bot/workbench/repos/tablec
git add .github/workflows/publish-python.yml
git commit -m "ci(python): add publish-python workflow — wheel on tag, attach to release"
```

---

## Task 4: 端到端 wheel 验证 — 干净 venv 装 wheel + 跑 pytest

**Files:**
- 不改文件;只是 end-to-end 验证,产物会写到验证脚本输出,无 commit

**Interfaces:**
- Consumes: Task 1 改完的 pyproject.toml + `cargo build` 出的 .so 通过 maturin
- Produces: 一个能在干净 venv 里 `pip install` 的 wheel + 跑通 pytest 的证据(用于 PR 描述 / beads close 注释)

### Steps

- [ ] **Step 4.1:在干净 shell 里 build wheel**

```bash
cd /home/bot/workbench/repos/tablec/binding-python
rm -rf .venv-test dist/wheels
uv venv .venv-build --python 3.11
source .venv-build/bin/activate
uv pip install maturin pytest openpyxl
maturin build --release --strip --out dist/wheels
ls -lh dist/wheels/
```

预期:`dist/wheels/` 下出现一个 `.whl`,文件名前缀是 `tablec-0.1.0-`(用 `manylinux2014` 会再带 `-manylinux_2_17_x86_64.manylinux2014_x86_64` 后缀;`uv run maturin build` 不加这个 flag 时只会有 cp310-abi3-linux_x86_64 后缀)。两种后缀都算通过。

如果报 `cargo build` 失败:检查 workspace `Cargo.toml` 的 `members` 是否包含 `binding-python`(已确认包含),并确认 `cargo build --release` 在 binding-python 里能跑通。

- [ ] **Step 4.2:在**完全独立**的 venv 里装 wheel(不能用 build 用的 venv)**

```bash
cd /home/bot/workbench/repos/tablec/binding-python
deactivate 2>/dev/null || true
rm -rf .venv-consumer
uv venv .venv-consumer --python 3.11
source .venv-consumer/bin/activate
uv pip install openpyxl pytest
uv pip install --force-reinstall dist/wheels/*.whl
```

预期:`uv pip install` 成功,`Successfully installed tablec-0.1.0-...`。

- [ ] **Step 4.3:从装好的 wheel 验证 import 名字 + 版本 + check() 跑通**

```bash
source .venv-consumer/bin/activate
python -c "import tablec; print('version:', tablec.__version__); print('module:', tablec.__file__)"
```

预期:
```
version: 0.1.0
module: /home/bot/workbench/repos/tablec/binding-python/.venv-consumer/lib/python3.11/site-packages/tablec/__init__.py
```

> 关键验收:`import tablec` 成功 — 验证 Task 1 把 package name 改成 `tablec`、import name 也是 `tablec` 这条主线没断。

- [ ] **Step 4.4:跑表 1 中的测试在装好的 venv 里**

```bash
source .venv-consumer/bin/activate
cd /home/bot/workbench/repos/tablec/binding-python
pytest tests/ -v
```

预期:全绿(3 个 test_*.py,8 个测试用例左右)。这验证 wheel 装出来的 `tablec` 与 `maturin develop` 出来的等价。

- [ ] **Step 4.5:清理 venv(可选,只是为了不污染 working tree)**

```bash
cd /home/bot/workbench/repos/tablec/binding-python
deactivate 2>/dev/null || true
rm -rf .venv-build .venv-consumer
ls
```

预期:不出现 `.venv-build` / `.venv-consumer`;`dist/wheels/*.whl` 留下作为"能 build 出 wheel"的证据,或一并 `rm -rf dist/` 也行(本 plan 不要求 commit 它们)。

> `dist/` 与 `binding-python/dist/` 都在 `.gitignore` 里吗?执行前 `cat .gitignore` 确认;如果不在,本步骤最后 `rm -rf binding-python/dist` 防止误提交。

---

## Task 5: 关闭 beads issues + 把分支推到远端

**Files:**
- 不改代码

**Interfaces:**
- Consumes: Task 1-4 全部完成
- Produces: `tablec-jbo` / `tablec-bh6` 关闭;feat 分支推到远端等用户合并

### Steps

- [ ] **Step 5.1:确认当前在 `feat/publish-python-spec` 分支**

```bash
cd /home/bot/workbench/repos/tablec
git branch --show-current
git status --short
```

预期:`feat/publish-python-spec`;`git status` 输出形如 `docs/superpowers/specs/...` untracked 或 commit 之外不应有未 commit 改动(`dist/` 之类被 gitignore 即可)。

- [ ] **Step 5.2:关闭 binding-python 这条(本地验证已通过)**

```bash
export PATH=/usr/local/bin:$PATH
cd /home/bot/workbench/repos/tablec
bd close tablec-bh6 --reason "pyproject 改名 + metadata 已加;本地 maturin build + pip install + pytest 全绿"
```

- [ ] **Step 5.3:关闭 publish-python 这条(workflow 已加,等 push tag 真跑过再确认,这里先 close 落地的代码工作)**

```bash
export PATH=/usr/local/bin:$PATH
cd /home/bot/workbench/repos/tablec
bd close tablec-jbo --reason "publish-python.yml 已加;明天 PyPI publish job 单独再起 issue"
```

- [ ] **Step 5.4:把分支推到远端(per push policy)**

```bash
cd /home/bot/workbench/repos/tablec
git pull --rebase
git push -u origin feat/publish-python-spec
```

预期:远端 `feat/publish-python-spec` 与本地同步。等用户 review 后合并到 main。

- [ ] **Step 5.5:把 beads 状态推到远端(bd dolt push)**

```bash
export PATH=/usr/local/bin:$PATH
cd /home/bot/workbench/repos/tablec
bd dolt push
```

预期:无 error。

---

## Self-Review

- ✅ Spec coverage:
  - §1.1 binding-python 当前问题 → Task 1 解决(改名 + metadata)
  - §1.2 release.yml 复用 → Task 3 attach-release job 走 `gh release upload` 到既有 release
  - §2.1 pyproject.toml 改动 → Task 1 Step 1.2 整文件给出
  - §2.2 Cargo.toml 不动 → 没有 Task 改它
  - §2.3 src/lib.rs 不动 → 没有 Task 改它
  - §2.4 README 改动 → Task 2 4 个 Edit 全列出
  - §2.5 本地验证 → Task 4 整段
  - §3 publish-python.yml 完整 YAML → Task 3 Step 3.2 整文件给出
  - §3.4 不在本 workflow(PyPI publish / macOS / Windows)→ §6 不在本 spec 范围 重申,plan 不写
- ✅ Placeholder scan:无 TBD / TODO / "add appropriate error handling";每个 step 都给了具体代码或命令
- ✅ 类型一致性:plan 里 `import tablec` / `tablec.check()` / `tablec.__version__` / `tablec._native` 都与 src/lib.rs 和 pyproject.toml 实际一致
- ✅ 命令可执行性:所有命令都用绝对路径或显式 `cd`,没有"参见 Task N"类的跨任务跳转
- ✅ 验收:`bd close` 写在 Task 5;push 走 feat 分支,符合 [[feedback-push-policy]]

执行入口建议:用 `superpowers:subagent-driven-development`(本 plan 5 个 task 都比较小,subagent 派发更清晰)。
