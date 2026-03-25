# AI Agents Guide

Core principles for AI coding assistants.
Compatible with the AGENTS.md open standard (Linux Foundation AAIF).

## Language & Communication

- Code, comments, documentation, commits, issues: **English only**
- Agent communication with user: **Spanish only**
- Mem0 entries: **Spanish only**
- Multilingual projects: **Must use i18n** (according to specs)

## Platform Defaults

- **GitLab** is the default for everything: repos, issues, MRs, CI/CD
- **GitHub** only when project has GitHub remote connected OR user explicitly requests it
- Prefer GitLab MCP → fallback `glab` CLI. For GitHub: GitHub MCP → `gh` CLI
- **This rule applies everywhere below** — not repeated per section

## Core Principles

- **Conventional Commits**: `<type>(<scope>): <subject>` — types: feat, fix, docs, style, refactor, test, chore
- **Naming**: Descriptive, self-documenting names
- **Comments**: Explain "why" not "what"
- **No Hacks**: No hardcoded values, workarounds, or magic numbers
- **Modify First**: Always modify existing files before creating new ones
- **No MD Generation**: Never create .md files unless explicitly requested
- **MCP Tools**: Use available MCP tools for every task (see MCP_TOOLS.md)
- **Context7**: Always use for library docs — never rely on training data
- **Mem0**: Save all important info with project context (userId = project name)
- **Verification**: Always verify changes before declaring completion
- **Workflow**: Issues → Mem0 → Git commits for every task
- **Project State Recovery**: When context is lost, check Issues + Mem0 + Git history

## Agent Autonomy

**Complete tasks from start to finish without stopping for confirmation.**

Do everything autonomously: create/modify/delete files, run tests, fix errors, commit, push, update docs, deploy. Never ask "should I continue?" or "shall I proceed?".

**Only ask confirmation for:**
- Deleting files outside project directory
- Changes to production systems/databases
- Modifying security configurations or system files

## REMOTE.md - Project Remote Configuration

**Every project MUST have a `REMOTE.md`** in the project root containing all remote deployment information.

**REMOTE.md is NEVER tracked in git** — add to `.gitignore` immediately.

**Must contain:**
- Remote server URLs and IPs
- SSH users and access method
- Systemd service names and status commands
- Project directories on remote (app, logs, config)
- Nginx virtual host configuration and hostname
- Database connection details (host, port, name, user)
- Environment variables and secrets
- Container registry URLs and image names
- Any project-specific deployment commands

**Template:**
```markdown
# Remote Configuration - [Project Name]

## Server
- Host: `project.example.com` / `10.0.0.x`
- SSH: `ssh user@host`
- App dir: `/opt/project/`
- Logs: `/var/log/project/`

## Services
- Systemd: `project-api.service`, `project-worker.service`
- Status: `sudo systemctl status project-api`
- Restart: `sudo systemctl restart project-api`

## Nginx
- Config: `/etc/nginx/sites-available/project.conf`
- Frontend: `/var/www/project/`
- Hostname: `project.example.com`

## Database
- Host: localhost / remote
- Port: 5432
- Name: project_db
- User: project_user

## Container
- Registry: `registry.example.com/project`
- Pull: `docker pull registry.example.com/project:latest`

## Environment
- `.env` location: `/opt/project/.env`
- Key variables: DB_URL, SECRET_KEY, API_KEY
```

**When starting work on any project:**
1. Check if REMOTE.md exists — if not, ask user for remote details and create it
2. Read REMOTE.md to understand deployment target
3. Save remote info to Mem0 for context persistence

## Deployment Workflow - MANDATORY

**All testing and deployment happens on remote servers, NEVER locally.**

### The Flow

```
Local: code → commit → push → tag
Remote: pull → build/restart → verify
```

### Rules

1. **Develop locally**: Write and modify code on local machine
2. **Commit and push**: Every change goes through git
3. **Tag for release**: Create git tag to trigger container build in CI/CD
4. **Deploy via git pull**: On remote server, always `git pull` — **never scp for code**
5. **scp only for specific files**: Config files, .env, certificates — not application code
6. **No local Docker**: Don't run containers locally — containers are for CI/CD and remote deployment
7. **No local testing servers**: Don't start dev servers locally for testing — test on remote
8. **Nginx for frontends**: Deploy frontend builds to remote Nginx, configure virtual hosts per project hostname

### Deployment Steps

```bash
# 1. Local: commit and push
git add -A && git commit -m "feat(api): add endpoint" && git push

# 2. Local: tag for container build
git tag -a v1.2.0 -m "Release v1.2.0" && git push --tags

# 3. Remote: pull and deploy (via SSH)
ssh user@remote "cd /opt/project && git pull && sudo systemctl restart project-api"

# 4. Remote: frontend (build locally, deploy to nginx)
npm run build  # local
scp -r dist/* user@remote:/var/www/project/  # scp OK for built assets
ssh user@remote "sudo systemctl reload nginx"

# 5. Remote: verify
ssh user@remote "curl -s http://localhost:8080/health"
```

### Nginx Hostname Setup

Each project gets its own hostname on the remote server:
- Config at `/etc/nginx/sites-available/project.conf`
- Symlink to `sites-enabled`
- Frontend served from `/var/www/project/`
- API proxied to backend service port
- Document hostname and ports in REMOTE.md

## Language Versions

Use latest stable:
- **Python**: 3.14+ with `uv` (mandatory). Use 3.13/3.12/3.11 for library compat (ML, crypto). Never `pip` directly.
- **Node.js**: LTS (24.x active, 22.x maintenance)
- **Go**: 1.26+ | **Rust**: 1.84+ | **TypeScript**: 5.9+ | **Java**: JDK 25 (21 also supported)

**Package management:**
- Always latest stable versions. Avoid beta/alpha unless requested.
- Document version pinning with reason in comments.

## Git Branching

- **`main`**: Production-ready code. Deploy from here. **Never merge untested code.**
- **`feature/<name>`**: New features. Test and validate on remote from this branch.
- **`fix/<name>`**: Bug fixes. Test on remote before merging.
- **`hotfix/<name>`**: Critical production fixes. MR to main, tag immediately.
- **All testing happens on branches** — deploy branch to remote, validate, then merge to main.
- **Only merge to main when**: all required features work, tests pass on remote, code is stable.
- Direct push to `main` only for trivial changes (typos, config).
- Always create MR for significant changes.
- Delete branch after merge.

## Code Quality

**Error Handling:**
- Always handle errors explicitly — no silent catches
- Use language-appropriate patterns: try/except (Python), Result (Rust), error returns (Go)
- Log errors with context (what failed, with what input)
- Return meaningful error messages to users

**Input Validation:**
- Validate all external input (API params, form data, CLI args)
- Sanitize for SQL injection, XSS, path traversal
- Validate types, ranges, and formats at boundaries

**Logging:**
- Use structured logging (JSON preferred)
- Levels: ERROR (failures), WARN (degraded), INFO (operations), DEBUG (development)
- Include: timestamp, level, message, context (request_id, user_id)
- Never log secrets, passwords, or tokens

**Testing:**
- Tests are **optional** unless explicitly requested
- When requested: unit tests for business logic, integration tests for APIs
- Run tests on remote server, not locally

## Task Management

**Create and track ALL tasks as issues.**

### Mandatory: Issues from Analysis

**Every project analysis or code review MUST produce issues** for all problems and improvements found. No analysis is complete without issues created.

- One issue per problem/improvement (don't bundle unrelated items)
- Each issue MUST have a priority label
- Include: what's wrong, where, and proposed fix
- Reference related code (file, line, function)
- Analysis summary as parent issue with checklist linking child issues

### Priority Labels
- `priority::critical` — fix immediately (security, data loss, broken prod)
- `priority::high` — fix same day (broken features, degraded performance)
- `priority::medium` — fix within week (tech debt, minor bugs)
- `priority::low` — backlog (improvements, nice-to-haves)

### Workflow
1. Create issue with priority label and acceptance criteria
2. Work on feature branch
3. One commit per logical change, reference issue: `Closes #456`
4. Create MR → merge → issue auto-closes
5. Tag → deploy → verify on remote

## UI Development

Use **Playwright** (testing/automation) and **Chrome DevTools** (debugging) for all UI work.
Verify: tests pass, no console errors, responsive works, accessibility passes.
See UI_TESTING.md for complete guide.

## Verification Checklist

Before declaring complete:
- [ ] Code works on remote server (not just locally)
- [ ] Tests pass (if applicable)
- [ ] Linting/formatting passes
- [ ] Issue created and updated
- [ ] Commit messages are descriptive
- [ ] Important decisions saved to Mem0
- [ ] REMOTE.md exists and is current
- [ ] Pipeline passes (if CI/CD configured)

## Specialized Guides

- **AGENTS_CONFIG.md** — Agent configs, MCP settings, prompt locations
- **MCP_TOOLS.md** — MCP tools (Context7, Mem0, Playwright, GitLab, etc.)
- **TASK_MANAGEMENT.md** — Issue workflow | **CICD.md** — Pipeline monitoring
- **SECURITY.md** — Secrets management | **UI_TESTING.md** — Playwright + DevTools
- **PYTHON.MD** / **TYPESCRIPT.MD** / **GO.MD** / **RUST.MD** / **JAVA.MD** — Language guides
- **REACT.MD** / **REACT_NATIVE.MD** / **ANGULAR.MD** — Framework guides
- **EDA.MD** / **BDS.MD** / **LLM.MD** — Architecture patterns
- **TESTING_ADV.MD** / **TROUBLESHOOTING.MD** — Testing & debugging
- **NEW_TASK.MD** / **TECHNICAL_DEBT.MD** / **GENERAL.MD** — Project management

---

**Last Updated**: 2026-03-22
**Maintained By**: Alfonso de la Guarda
**Total Agents**: 20 (17 local + 3 remote)
**MCP Servers**: 12 functional
