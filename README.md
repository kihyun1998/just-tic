# jtic

> 오늘 내가 코드를 얼마나 썼나/지웠나 — 한 줄로.

`jtic`은 현재 git 레포에서 **오늘(머신 로컬 타임존 자정~지금) author date 커밋들**의
`numstat`을 합산해 추가/삭제 줄 수를 한 줄로 보여주는 작은 CLI입니다.

```console
$ jtic
+127 -34 · 5 commits
```

`git log --since=midnight --numstat`을 치고 눈으로 합산하던 일을 한 명령으로 줄입니다.
"오늘"의 경계(타임존), 머지 커밋, rename 같은 함정은 알아서 처리합니다.

## 설치

[Releases](https://github.com/kihyun1998/just-tic/releases)에서 OS에 맞는 아카이브를
받아 압축을 풀고, 안의 `jtic`(Windows는 `jtic.exe`) 바이너리를 `PATH`에 둡니다.

| OS | 아카이브 |
|----|----------|
| Linux (x86_64) | `jtic-<버전>-x86_64-unknown-linux-gnu.tar.gz` |
| macOS (Intel) | `jtic-<버전>-x86_64-apple-darwin.tar.gz` |
| macOS (Apple Silicon) | `jtic-<버전>-aarch64-apple-darwin.tar.gz` |
| Windows (x86_64) | `jtic-<버전>-x86_64-pc-windows-msvc.zip` |

소스에서 직접 빌드하려면:

```console
$ cargo install --git https://github.com/kihyun1998/just-tic --bin jtic
```

## 사용법

```console
$ jtic
+127 -34 · 5 commits
```

오늘 아무것도 커밋하지 않은 날:

```console
$ jtic
+0 -0 · no commits today
```

(종료 코드는 `0` — "작업 없음"은 에러가 아닙니다.)

### `--json`

상태바(tmux/starship/polybar)나 `jq` 연동을 위한 기계 판독 출력:

```console
$ jtic --json
{"date":"2026-06-05","additions":127,"deletions":34,"commits":5}
```

```console
$ jtic --json | jq .additions
127
```

### 색

stdout이 **TTY**일 때만 추가는 초록, 삭제는 빨강으로 표시합니다. 파이프/리다이렉트면
색을 끄고, [`NO_COLOR`](https://no-color.org) 환경변수를 존중합니다.

```console
$ jtic | grep +   # 파이프 → ANSI 색 코드 없음
$ NO_COLOR=1 jtic # 색 끔
```

### 셸 자동완성 · man page

clap 정의에서 파생해 생성하므로 항상 최신 플래그와 동기화됩니다.

```console
$ jtic completions bash   # bash | zsh | fish | powershell | elvish
$ jtic man                # roff man page
```

설치 예시:

```bash
# bash 자동완성
jtic completions bash > ~/.local/share/bash-completion/completions/jtic

# man page
jtic man > ~/.local/share/man/man1/jtic.1
```

```powershell
# PowerShell 자동완성 (프로필에 추가)
jtic completions powershell | Out-String | Invoke-Expression
```

## "오늘"은 어떻게 정의되나

- **경계**: 머신 **로컬 타임존**의 `[오늘 자정, 지금)` 반열림 구간. 자정 정각 커밋은 포함, 현재 시각은 제외.
- **날짜 기준**: 커밋의 **author date**로 판정합니다(committer date 아님). 어제 짠 걸 오늘 rebase/amend해도 어제 작업이 오늘로 부풀지 않습니다.
- **커밋 집합**: 로컬 브랜치(`refs/heads/*`) 전체에서 닿는 커밋을 모아 commit id로 중복 제거합니다. 오전엔 feature, 오후엔 master에 커밋한 날 둘 다 합산됩니다. `refs/remotes/*`(원격 추적 ref)는 제외해 `fetch`가 숫자를 부풀리지 않습니다.
- **머지 커밋**: 집계에서 제외합니다(`git log --no-merges`와 동일). 머지가 가져온 줄이 원본 커밋과 두 번 세지지 않게.
- **rename**: 유사도 감지를 켭니다. 순수 이동은 `0/0`, 이동+수정은 변경분만 잡혀 디렉터리 이동이 `+500 -500`으로 튀지 않습니다.
- **바이너리/gitignore**: 바이너리 파일은 `0` 기여, gitignore된 파일은 애초에 집계에 들어오지 않습니다.
- **워킹트리**: 커밋만 셉니다. staged/unstaged 미커밋 변경은 (v1에선) 제외합니다.

## 범위 밖 (v1)

`--mine`(author 필터), 미커밋 변경 집계, lock/generated 잡음 필터, committer-date 모드,
날짜 범위 지정(어제·이번 주), 설정 파일 등은 의도적으로 v1에서 제외했습니다.

## 라이브러리로 쓰기

모든 집계 로직은 코어 lib(`just_tic`)에 있고, CLI는 시계를 읽어 주입하는 얇은 껍데기입니다.
공개 API는 함수 하나입니다:

```rust
pub fn tally(repo: &gix::Repository, now: jiff::Zoned) -> anyhow::Result<Tally>;
```

코어는 시계를 직접 읽지 않고 `now`를 인자로 받습니다 — 그래서 자정 경계·DST를 고정 시각으로
결정적으로 테스트할 수 있고, 같은 로직을 GUI(예: 미래의 Tauri 프런트엔드)에서 그대로 재사용할 수 있습니다.

## 라이선스

[MIT](LICENSE-MIT) 또는 [Apache-2.0](LICENSE-APACHE) 중 선택해서 사용할 수 있습니다.
