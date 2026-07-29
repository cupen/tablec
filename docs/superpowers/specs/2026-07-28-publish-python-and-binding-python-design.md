# binding-python 打包修正 + publish-python.yml — 设计稿

**日期**: 2026-07-28
**仓库**: `repos/tablec`
**beads**: `tablec-jbo` (publish-python.yml) / `tablec-bh6` (binding-python 修正)
**触发**:
- `bd create` 记录的两项任务,用户要求 `pip install` 路径今天可走通
- 用户确认:包名 = import 名 = `tablec`;版本从 pyproject.toml 读;tag 与 version 手工保证一致;仅 Linux x86_64 wheel 走 CI;本地不验证 PyPI(明天账号)

**范围**:
- `binding-python/pyproject.toml` — name / description / authors / license / urls / classifiers
- `binding-python/README.md` — typo 修正、import 名称说明
- `.github/workflows/publish-python.yml` — 新增
- `.github/workflows/ci.yml` — 不动
- `.github/workflows/release.yml` — 不动

**不做**:
- 不发 PyPI(明天再写 publish job)
- macOS / Windows wheel(暂仅 Linux manylinux2014 x86_64)
- 不改 `binding-python/src/lib.rs` 公开 API
- 不改 Cargo workspace 结构
- 不加 cibuildwheel(用户明确选 maturin 直接 build)

---

## 1. 背景与目标

### 1.1 binding-python 当前问题

`binding-python/pyproject.toml` 里 `[project].name = "tablec-python"`,而 import 名为 `tablec`(maturin `module-name = "tablec._native"`)。PEP 421 允许 import 名称与项目名不同,但:

- `pip install tablec-python` → `import tablec` 体验割裂
- 真实使用场景(本地 venv + 服务器)用户会困惑:"我装的到底叫什么"
- PyPI 上 `tablec-python` 这个长名会跟未来其他工具撞名(类似 `python-foo` 模式)

同时 metadata 缺失:`description=空`、`authors=空`、`license=空`、没有 urls。pip / PyPI 显示的信息基本为空。

### 1.2 release.yml 当前状态

`.github/workflows/release.yml` 已经在 `push tag v*` 时:
1. 在 `cupen/test` 仓库跑 integration tests(通过 `repository_dispatch`)
2. 用三台 runner(Linux / Windows / macOS)build Rust binary
3. attach 到 GitHub Release

但它**不构建 Python wheel**。所以今天需要补一个 `publish-python.yml` 走 wheel 流程。

---

## 2. binding-python 改动

### 2.1 `pyproject.toml`

```toml
[build-system]
requires = ["maturin>=1.9.0,<2.0"]
build-backend = "maturin"

[project]
name = "tablec"                                                # was: tablec-python
version = "0.1.0"
description = "Python bindings for tablec — a Rust-based table compiler for gamedev (Excel/CSV/JSON -> JSON/MessagePack)"
readme = "README.md"
requires-python = ">=3.10"
license = "MIT"
license-files = ["../LICENSE"]                                 # workspace 根的 LICENSE
authors = [
    { name = "cupen", email = "xcupen@gmail.com" },
]
keywords = ["tablec", "excel", "gamedev", "table", "compiler", "msgpack"]
classifiers = [
    "Development Status :: 3 - Alpha",
    "Intended Audience :: Developers",
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
strip = true                                                   # wheel 内 .so 体积更小

[dependency-groups]
dev = [
    "pytest>=8.4.1",
    "openpyxl>=3.1.5",
]
```

`license-files = ["../LICENSE"]`:maturin 0.x / setuptools-scm 对 workspace LICENSE 的相对路径处理。这里 `binding-python/../LICENSE` 即 repo 根的 `LICENSE`(MIT)。

### 2.2 `Cargo.toml`

不动。`name = "binding-python"` 是 crate 内部名,跟 PyPI 包名无关,不需要改。`pyo3` 的 features 列表里同时打开 `abi3-py310..abi3-py313` 是冗余写法(只取一个就够),但 `cargo build` / `maturin build` 不报错,改它属于另外的 spec。

### 2.3 `src/lib.rs`

不动。`build` / `check` 已经满足 README 中的接口。

### 2.4 `README.md`

修正 + 调整(行号以当前版本为准):
- `uv matisin develop` → `uv run maturin develop`(typo 修正;`uv run` 走 venv)
- `### From wheel (future)` 段:`pip install tablec-python` → `pip install tablec`
- `## Building for Distribution` 段:`uv maturin build` → `uv run maturin build`(用 venv 中的 maturin,避免全局污染)
- `uv maturin build --release --strip --compatibility manylinux` 这行保留(本地可手动 build 跨发行版 wheel;CI 的 manylinux 走 maturin 自带的 `manylinux2014` Docker 镜像,不依赖这行)
- 加一节"## Import name vs package name":一句话说明 `pip install tablec` 然后 `import tablec`,避免读者疑惑

### 2.5 验证(本地)

```bash
cd binding-python
uv venv .venv-test --python 3.11
source .venv-test/bin/activate
uv pip install maturin pytest openpyxl
maturin build --release --strip                                  # 本地默认走 host glibc
ls dist/                                                          # 应有一个 .whl
uv pip install --force-reinstall dist/tablec-0.1.0-*.whl
python -c "import tablec; print(tablec.__version__)"             # 0.1.0
python -c "import tablec; tablec.check('examples/testdata/.../*.xlsx')"   # smoke
pytest tests/ -v                                                  # 跑现有测试
```

成功判据:`tablec.check` 不抛异常,pytest 全绿。

> 注:本地 build 默认用 host glibc(Linux 2.31+),只在本机可装。要做跨发行版 wheel,加 `--compatibility manylinux2014`(maturin 拉 Docker 镜像,本地需要 docker);CI 工作流里默认加。

---

## 3. publish-python.yml

### 3.1 文件

`.github/workflows/publish-python.yml`(新文件)。

### 3.2 设计要点

- **触发**:`push tags: 'v*'` + `workflow_dispatch`
- **运行平台**:`ubuntu-latest`(只出 manylinux2014 x86_64 wheel;macOS / Windows 暂不做)
- **build job**:`maturin build --release --strip`,产出 wheel,upload artifact
- **attach job**:`gh release upload <tag> <wheel> --clobber`,附加到 release.yml 已创建好的 GitHub Release
- **race 缓解**:`attach-release` job 在 `gh release upload` 前轮询 release 是否存在(40 × 15s = 10 分钟),超时则报错
- **PyPI publish job**:**本 spec 不做**(明天再写;预留 secrets 文档)

### 3.3 文件内容

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
            # 手动触发:用上一次 push 的 tag
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

### 3.4 不在本 workflow 的部分

- **PyPI publish job**:本 spec 故意不写,留到明天有账号再写。`README.md` 在 "Python Bindings / Building for Distribution" 节里加一句"明天(2026-07-29)起 CI 会自动推 PyPI,见 `.github/workflows/publish-python.yml`",作为占位提示
- **macOS arm64 / Windows x86_64 wheel**:本期不做,需要的话另起一个 `build-wheel-matrix` job

---

## 4. 决策摘要

| 决策 | 选择 | 否决方案 |
|------|------|----------|
| PyPI 包名 | `tablec`(与 import 名一致) | 保留 `tablec-python`(体验割裂) |
| wheel 目标平台 | only Linux x86_64 | 三平台全上(用户偏好精简) |
| 构建工具 | `maturin build` 直接跑 | cibuildwheel(用户偏好简单) |
| version 来源 | pyproject.toml 读 | 从 git tag 推(用户偏好手工) |
| Release 渠道 | 复用 release.yml 创建的 Release | 单独建 release(双发布混乱) |
| race 处理 | `attach-release` job 轮询 10 min | 用 `softprops/action-gh-release` 替换 release.yml(侵入) |
| PyPI 上传 | 本 spec 不做 | 一起做(没有账号做不了) |
| README `maturin develop` 用法 | `uv run maturin develop` | `uv pip install maturin && maturin develop`(无版本锁定) |

---

## 5. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| PyPI 名 `tablec` 已被占用 | publish 失败 | 明天账号上 PyPI 后查,被占则降级为 `tablec-cli` / 加后缀 |
| `attach-release` 轮询超时 | workflow 失败 | 10 分钟足够 release.yml build(实测 Linux ~2 min,Windows ~3 min,macOS ~3 min,串行 ~8 min);`workflow_dispatch` 路径已独立处理 |
| `license-files = ["../LICENSE"]` 路径解析失败 | wheel 元数据丢失 | maturin 1.9+ 支持相对路径;`uv run maturin build` 本地先验证 |
| abi3 多 feature(`abi3-py310..abi3-py313`) 编译警告 | wheel 工作但日志吵 | 本 spec 不动 Cargo.toml(与本期目标无关);下个 spec 收 |
| `swatinem/rust-cache` workspace 路径 | cache key 失效 | 配 `workspaces: ./binding-python -> target` 让它感知 workspace root |
| `maturin build` 不带 `--compatibility manylinux` | wheel 只能跑在 ≥ host glibc 的系统上,musl/旧发行版用不了 | CI 显式加 `--compatibility manylinux2014`(maturin 自带 Docker 镜像);本地验证可以省略,因为只在本机装 |
| `examples/testdata` 路径硬编码 | 本地验证脚本脆 | 验证脚本用 git ls-files 或 `find` 找一个真实 xlsx;不写死路径 |

---

## 6. 不在本 spec 范围

- PyPI publish(明天再说)
- macOS arm64 / Windows x86_64 wheel
- `binding-python::build()` 暴露更多 API
- cibuildwheel 切换
- `Cargo.toml` abi3 features 收敛
- 升级 pyo3 / maturin 主版本
- CI 缓存(本期只走 rust-cache 即可)

---

## 7. 落地节奏

按"先 binding-python 验证,后 publish-python.yml"的顺序:

```
0. beads: bd list                              # 确认 tablec-jbo / tablec-bh6 存在
1. 改 binding-python/pyproject.toml (tablec-bh6)
2. 改 binding-python/README.md (typo + 包名 + 新增"Import name vs package name"节)
3. 本地:cd binding-python && uv venv && maturin build && pip install && import test
4. bd close tablec-bh6                         # 本地验证通过
5. 新增 .github/workflows/publish-python.yml (tablec-jbo)
6. 本地:actionlint .github/workflows/publish-python.yml   (如有 actionlint)
7. 在当前 feat/publish-python-spec 分支上继续 commit,push 后由用户合并 main
8. bd close tablec-jbo                         # 等 spec 落地后再 close
```

落地时按 beads 走;`git push` 走 feat 分支,等用户合并 main。

---

## 8. 附录

### 8.1 与现有 spec / 计划的关系

- `2026-07-25-tablec-cli-simplification-design.md`:本 spec 是其下游,不动 CLI
- `2026-07-26-build-dir-design.md`:不重叠(那个是 CLI build 子命令目录支持,本期是 Python 打包)
- `.github/workflows/release.yml`(已存在):本 spec 复用的对象,不改

### 8.2 验证清单(交付前)

- [ ] `bd list` 看到 `tablec-jbo` / `tablec-bh6` 两条 open issue
- [ ] `pyproject.toml` 用 `python -c "import tomllib; tomllib.load(open('pyproject.toml','rb'))"` 解析无错
- [ ] 本地 `maturin build --release` 产出 `tablec-0.1.0-cp310-abi3-manylinux_2_31_x86_64.whl`(或类似)
- [ ] 本地 `pip install dist/*.whl` 后 `python -c "import tablec; print(tablec.__version__)"` 输出 `0.1.0`
- [ ] 本地 `tablec.check(...)` 跑通
- [ ] 本地 `pytest tests/` 全绿
- [ ] `actionlint .github/workflows/publish-python.yml`(如有)无错
- [ ] `bd close tablec-bh6` / `bd close tablec-jbo`

### 8.3 references

- skill: `superpowers:brainstorming` — 本 spec 由该 skill 流程产出
- skill: `superpowers:writing-plans` — 由该 skill 转 implementation plan
- 现有: `.github/workflows/release.yml`(reused,不改)
- maturin 文档: <https://www.maturin.rs/>
- PEP 421: <https://peps.python.org/pep-0421/>
- pyo3 abi3: <https://pyo3.rs/v0.27.2/feature-flags#abi3>
