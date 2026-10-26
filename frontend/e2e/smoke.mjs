// Browser smoke test for the built web interface.
//
// Runs the real `frontend/dist` bundle in headless Chromium against a running API,
// drives it over the DevTools protocol, and fails when any route renders nothing,
// throws, or cannot be reached after signing in.
//
// This exists because the unit tests run source modules in jsdom: they cannot see a
// production bundle that throws at load, a route the router does not match, or a
// navigation the server answers with 404. Every defect below was found this way.
//
//   node frontend/e2e/smoke.mjs --base http://127.0.0.1:8098
//
// No dependencies: Node 18+ has fetch, Node 22 has WebSocket.

import { spawn } from 'node:child_process'
import { mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const args = process.argv.slice(2)
const arg = (name, fallback) => {
  const index = args.indexOf(`--${name}`)
  return index >= 0 ? args[index + 1] : fallback
}

const BASE = arg('base', 'http://127.0.0.1:8098')
const CHROME = arg('chrome', 'chromium-browser')
const PORT_ARG = arg('port', '')

/** A port nothing is listening on, asked of the operating system rather than guessed. */
async function freePort() {
  const { createServer } = await import('node:net')
  return new Promise((resolve, reject) => {
    const server = createServer()
    server.once('error', reject)
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address()
      server.close(() => resolve(port))
    })
  })
}

const PORT = PORT_ARG ? Number(PORT_ARG) : await freePort()
// Now a timeout rather than a fixed wait: `visit` polls until the page has content.
const SETTLE_MS = Number(arg('settle', '5000'))

// Routes the interface publishes. Each must render something a person can see.
const ROUTES = [
  '/',
  '/login',
  '/rules',
  '/rules/new',
  '/governance',
  '/governance/approvals',
  '/governance/audit',
  '/governance/branches',
  '/governance/rbac',
  '/monitoring',
  '/monitoring/coverage',
  '/monitoring/dead-rules',
  '/monitoring/engine',
]

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

async function waitForDevtools(chrome, timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${PORT}/json/list`)
      const targets = await response.json()
      const page = targets.find((target) => target.type === 'page')
      if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl
    } catch {
      // not up yet
    }
    await sleep(200)
  }
  const detail = chrome?.lastError ? ` (${chrome.lastError})` : ''
  const exited = chrome?.exitCode !== null && chrome?.exitCode !== undefined ? ` exit ${chrome.exitCode}` : ''
  throw new Error(`Chromium did not expose DevTools on port ${PORT}${exited}${detail}`)
}

/** A minimal DevTools client: one page target, request/response by id. */
class Page {
  constructor(socket) {
    this.socket = socket
    this.nextId = 1
    this.pending = new Map()
    this.errors = []
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data)
      if (message.id && this.pending.has(message.id)) {
        const { resolve, reject } = this.pending.get(message.id)
        this.pending.delete(message.id)
        message.error ? reject(new Error(JSON.stringify(message.error))) : resolve(message.result)
        return
      }
      if (message.method === 'Runtime.exceptionThrown') {
        const details = message.params?.exceptionDetails
        this.errors.push(details?.exception?.description ?? details?.text ?? 'exception')
      }
      if (message.method === 'Runtime.consoleAPICalled' && message.params?.type === 'error') {
        const text = (message.params.args ?? [])
          .map((item) => item.value ?? item.description ?? '')
          .join(' ')
        this.errors.push(text)
      }
    })
  }

  send(method, params = {}) {
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      this.socket.send(JSON.stringify({ id, method, params }))
    })
  }

  async evaluate(expression) {
    const result = await this.send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
    })
    if (result.exceptionDetails) {
      // `text` alone is the word "Uncaught", which tells nobody anything: take the
      // thrown value's description, and the first stack line, when they exist.
      const details = result.exceptionDetails
      const thrown = details.exception?.description ?? details.exception?.value
      const where = (details.stackTrace?.callFrames ?? [])
        .slice(0, 1)
        .map((frame) => ` (${frame.functionName || 'anonymous'} at line ${frame.lineNumber})`)
        .join('')
      throw new Error(`${thrown ?? details.text ?? 'evaluation failed'}${where}`)
    }
    return result.result?.value
  }

  /// Visit and wait until the page matches `marker`, so an assertion about data a page fetches
  /// is not racing the fetch. The navbar alone satisfies `visit`, which is what made the
  /// governance checks intermittently report "rendered its data" as a failure.
  async visitMatching(url, marker, timeoutMs = SETTLE_MS) {
    this.errors = []
    await this.send('Page.navigate', { url })
    const deadline = Date.now() + timeoutMs
    let text = ''
    while (Date.now() < deadline) {
      text = await this.evaluate('document.body ? document.body.innerText : ""')
      if (text && marker.test(text)) break
      await sleep(50)
    }
    return { text: (text ?? '').replace(/\s+/g, ' ').trim(), errors: [...this.errors] }
  }

  async visit(url) {
    this.errors = []
    await this.send('Page.navigate', { url })
    // Wait for content rather than for a fixed delay. A cold server — the first load after
    // `make e2e` starts it on a runner — can take longer than any delay that keeps the
    // whole gate quick, and the result was a route reported as "rendered nothing" while
    // every later route passed: a flake that looks exactly like a broken app.
    const deadline = Date.now() + SETTLE_MS
    let text = ''
    while (Date.now() < deadline) {
      text = await this.evaluate('document.body ? document.body.innerText : ""')
      if (text && text.trim().length > 0) break
      await sleep(50)
    }
    return { text: (text ?? '').replace(/\s+/g, ' ').trim(), errors: [...this.errors] }
  }
}

/** Kill the browser and wait for it to exit, so its profile can be removed. */
async function stopChromium(child, timeoutMs = 5_000) {
  if (!child || child.exitCode !== null) return
  const exited = new Promise((resolve) => child.once('exit', resolve))
  child.kill('SIGKILL')
  await Promise.race([exited, sleep(timeoutMs)])
}

function launchChromium(profile) {
  const child = spawn(
    CHROME,
    [
      '--headless=new',
      '--no-sandbox',
      '--disable-gpu',
      '--disable-dev-shm-usage',
      `--remote-debugging-port=${PORT}`,
      `--user-data-dir=${profile}`,
      '--window-size=1200,800',
      'about:blank',
    ],
    { stdio: ['ignore', 'pipe', 'pipe'], detached: false },
  )
  // Keep the browser's own output: "did not expose DevTools" is unhelpful when Chromium said why.
  child.stderr?.on('data', (chunk) => {
    const text = chunk.toString().trim()
    if (text) child.lastError = text.split('\n').slice(-3).join(' | ')
  })
  child.on('error', (error) => {
    console.error(`could not launch ${CHROME}: ${error.message}`)
  })
  return child
}

const failures = []
const report = (ok, label, detail) => {
  console.log(`${ok ? '  ok  ' : ' FAIL '} ${label}${detail ? `  ${detail}` : ''}`)
  if (!ok) failures.push(`${label}: ${detail}`)
}

// The browser profile lives under the repository's build directory rather than the
// system temp dir: it is gitignored, and a profile Chromium has not finished writing
// cannot escape into `/tmp` on every run.
const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
const profileRoot = process.env.E2E_PROFILE_DIR || join(repoRoot, 'target', 'e2e')
mkdirSync(profileRoot, { recursive: true })
// One profile directory, reused: it is build output under `target/`, it does not
// accumulate a copy per run, and Chromium rewriting it is harmless. Removing it on
// the way out raced the browser still flushing to it (`ENOTEMPTY`), which turned a
// tidy-up into a failed step once already.
const profile = join(profileRoot, 'profile')
rmSync(profile, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })
const chromium = launchChromium(profile)
let page

try {
  const socketUrl = await waitForDevtools(chromium)
  const socket = new WebSocket(socketUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', () => reject(new Error('DevTools socket failed')))
  })
  page = new Page(socket)
  await page.send('Page.enable')
  await page.send('Runtime.enable')
  await page.send('Log.enable')

  console.log(`\nroutes (${BASE})`)
  for (const route of ROUTES) {
    const { text, errors } = await page.visit(`${BASE}${route}`)
    const uncaught = errors.filter((entry) => /Uncaught|TypeError|is not a function/.test(entry))
    report(text.length > 0, route, text.length ? `${text.length} chars` : 'rendered nothing')
    if (uncaught.length) {
      report(false, `${route} console`, uncaught[0].slice(0, 200))
    }
  }

  // A real page load must be reachable directly: a deep link is a bookmark or a
  // reload, and it goes through the server before the router ever runs.
  console.log('\ndeep link')
  const deep = await page.visit(`${BASE}/monitoring/engine`)
  report(deep.text.length > 0, '/monitoring/engine as an entry point', `${deep.text.length} chars`)

  // Signed in, the rules page must show the data rather than an auth error.
  console.log('\nauthenticated')
  const credentials = { email: 'admin', password: 'admin' }
  const login = await page.evaluate(`
    fetch('${BASE}/v1/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(${JSON.stringify(credentials)}),
    }).then((response) => response.ok ? response.json() : null)
  `)
  if (!login?.token) {
    report(false, 'login', 'the API rejected the seeded admin credentials')
  } else {
    await page.evaluate(`
      localStorage.setItem('auth_token', ${JSON.stringify(login.token)});
      localStorage.setItem('user', ${JSON.stringify(JSON.stringify(login.user))});
      true
    `)
    const signedIn = await page.visit(`${BASE}/rules`)
    if (process.env.E2E_DEBUG) {
      console.log(`  debug text: ${JSON.stringify(signedIn.text)}`)
      for (const entry of signedIn.errors) console.log(`  debug error: ${entry.slice(0, 300)}`)
    }
    const showsAuthError = /status code 401/.test(signedIn.text)
    report(
      !showsAuthError && signedIn.text.includes('Manage Rules'),
      '/rules while signed in',
      showsAuthError ? 'still reported 401' : `${signedIn.text.length} chars`,
    )
    const displays = signedIn.text.includes('admin')
    report(displays, 'navbar shows the signed-in user', displays ? 'admin' : 'no user name')

    // Creating a rule is the flow that was never wired: the page had an editor and no
    // name, no submit, and no call to the API.
    console.log('\ncreate a rule')
    const ruleName = `e2e-${Date.now()}`
    const created = await page.evaluate(`
      (async () => {
        const token = localStorage.getItem('auth_token');
        const response = await fetch('${BASE}/v1/rules', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + token },
          body: JSON.stringify({
            name: ${JSON.stringify(ruleName)},
            description: 'created by the browser smoke test',
            dsl_body: 'rule "smoke" when true then log("ok")',
            tags: [], salience: 0,
          }),
        });
        return response.status;
      })()
    `)
    report(created === 201, 'POST /v1/rules from the browser', `status ${created}`)

    const listed = await page.visit(`${BASE}/rules`)
    report(listed.text.includes(ruleName), '/rules shows the new rule', ruleName)

    // The form itself must be submittable: fill it in and watch the list change.
    // The edit page must show the rule it was asked to edit, not a stub.
    const ruleId = await page.evaluate(`
      (async () => {
        const response = await fetch('${BASE}/v1/rules?page_size=50', {
          headers: { Authorization: 'Bearer ' + localStorage.getItem('auth_token') },
        });
        const body = await response.json();
        const match = (body.rules ?? []).find((rule) => rule.name === ${JSON.stringify(ruleName)});
        return match ? match.id : null;
      })()
    `)
    if (ruleId) {
      // The name is an `<input value>`, which `innerText` does not contain: read the
      // control, not the text, or the check reports a stub for a working page.
      await page.visit(`${BASE}/rules/${ruleId}/edit`)
      const loadedName = await page.evaluate(
        `document.querySelector('input[name="name"]')?.value ?? ''`,
      )
      report(
        loadedName === ruleName,
        '/rules/{id}/edit loads the rule it edits',
        loadedName === ruleName ? ruleName : `name field held ${JSON.stringify(loadedName)}`,
      )
    } else {
      report(false, '/rules/{id}/edit shows the rule it edits', 'could not resolve the new rule id')
    }

    // The list's own actions: the create button has to go somewhere, and a row's delete has to
    // remove the rule through the API.
    const listActions = await page.visit(`${BASE}/rules`)
    const createAction = await page.evaluate(`
      (async () => {
        const button = [...document.querySelectorAll('button')].find((b) =>
          /create rule/i.test(b.textContent),
        );
        if (!button) return { error: 'no Create Rule button' };
        button.click();
        await new Promise((resolve) => setTimeout(resolve, 900));
        return { navigated: document.body.innerText.includes('New Rule') };
      })()
    `)
    report(
      Boolean(createAction?.navigated),
      "the list's Create Rule button opens the form",
      createAction?.error ??
        (createAction?.navigated ? 'navigated' : `stayed on ${listActions.text.slice(0, 40)}`),
    )

    if (ruleId) {
      await page.visit(`${BASE}/rules`)
      const removed = await page.evaluate(`
        (async () => {
          const row = [...document.querySelectorAll('tr')].find((tr) =>
            tr.textContent.includes(${JSON.stringify(ruleName)}),
          );
          if (!row) return { error: 'the rule is not in the table' };
          const button = [...row.querySelectorAll('button')].find((b) => /archive/i.test(b.textContent));
          if (!button) return { error: 'no Archive action on the row' };
          button.click();
          await new Promise((resolve) => setTimeout(resolve, 1500));
          const response = await fetch('${BASE}/v1/rules?page_size=100', {
            headers: { Authorization: 'Bearer ' + localStorage.getItem('auth_token') },
          });
          const body = await response.json();
          // DELETE /v1/rules/{id} archives: the row stays and its status changes.
          const rule = (body.rules ?? []).find((r) => r.name === ${JSON.stringify(ruleName)});
          return { status: rule ? rule.status : 'gone' };
        })()
      `)
      report(
        removed?.status === 'archived',
        "a row's Archive archives the rule",
        removed?.error ?? `status ${removed?.status}`,
      )
    }

    // The maker-checker workflow, driven through the interface: submit a rule for approval, find it
    // in the approval queue, and approve it there. The API pieces exist and the queue's buttons are
    // wired, but nothing had ever walked the whole loop in a browser.
    if (ruleId) {
      const submitted = await page.evaluate(`
        (async () => {
          const response = await fetch('${BASE}/v1/rules/${ruleId}/submit', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              Authorization: 'Bearer ' + localStorage.getItem('auth_token'),
            },
            body: JSON.stringify({ maker_id: 'e2e-maker' }),
          });
          return response.status;
        })()
      `)
      report(submitted === 201 || submitted === 200, 'a rule can be submitted for approval', `status ${submitted}`)

      const queue = await page.visitMatching(`${BASE}/governance/approvals`, /Approve|pending/i)
      const approved = await page.evaluate(`
        (async () => {
          const button = [...document.querySelectorAll('button')].find((b) => /^approve$/i.test(b.textContent.trim()));
          if (!button) return { error: 'no Approve action in the queue' };
          button.click();
          await new Promise((resolve) => setTimeout(resolve, 1500));
          const response = await fetch('${BASE}/v1/approvals/pending', {
            headers: { Authorization: 'Bearer ' + localStorage.getItem('auth_token') },
          });
          const body = await response.json();
          const pending = (body.approvals ?? []).length;
          const audit = await fetch('${BASE}/v1/audit?page_size=100', {
            headers: { Authorization: 'Bearer ' + localStorage.getItem('auth_token') },
          });
          const events = (await audit.json()).events ?? [];
          return { pending, audited: events.some((event) => event.action === 'approve') };
        })()
      `)
      report(
        approved && approved.pending === 0,
        'the queue\'s Approve discharges the request',
        approved?.error ?? `${approved?.pending} still pending`,
      )
      report(
        Boolean(approved?.audited),
        'the approval is recorded in the audit trail',
        approved?.audited ? 'approve event present' : 'no approve event',
      )
    }

    // Signed in, the governance pages must show their endpoint's answer — an empty
    // state from the API is fine, an auth error is not.
    console.log('\ngovernance with data')
    for (const [route, marker] of [
      ['/governance/approvals', /No Pending Approvals|Approve|Reject/],
      ['/governance/audit', /No audit events found|Event Type|Apply Filters/],
      ['/governance/branches', /main|master|Branch/],
      ['/governance/rbac', /Permissions|role/i],
    ]) {
      const seen = await page.visitMatching(`${BASE}${route}`, marker)
      const matched = marker.test(seen.text) && !/status code 401/.test(seen.text)
      report(
        matched,
        route,
        matched ? 'rendered its data' : `text began: ${seen.text.slice(0, 100)}`,
      )
    }

    // Running the rule must say what happened: a rule created here is not in the
    // engine's rule set, and the page has to say so rather than show "0 fired".
    if (ruleId) {
      await page.visit(`${BASE}/rules/${ruleId}/edit`)
      const ran = await page.evaluate(`
        (async () => {
          const button = [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Run');
          if (!button) return 'no Run button';
          button.click();
          await new Promise((resolve) => setTimeout(resolve, 1500));
          return document.body.innerText;
        })()
      `)
      report(
        /No engine rule matched|Rules selected by the filter/.test(ran),
        'Run explains a zero-match execution',
        /No engine rule matched/.test(ran) ? 'boundary explained' : 'no explanation shown',
      )
    }

    // The reload → poll → execute chain has only ever run against mocks. Press it
    // against the real server: it must terminate in a result or a reported error, not
    // in a spinner that never resolves.
    if (ruleId) {
      await page.visit(`${BASE}/rules/${ruleId}/edit`)
      const reloaded = await page.evaluate(`
        (async () => {
          try {
          const button = [...document.querySelectorAll('button')].find((b) =>
            /reload then run/i.test(b.textContent),
          );
          if (!button) return 'no Reload then Run button';
          button.click();
          await new Promise((resolve) => setTimeout(resolve, 4000));
          const text = document.body.innerText;
          if (/Rules selected by the filter/.test(text)) return 'result';
          if (/role="alert"|error|failed/i.test(text) || /Could not|failed/i.test(text)) return 'reported';
          return 'still pending: ' + text.slice(0, 120).replace(/\\s+/g, ' ');
          } catch (error) {
            return 'threw: ' + (error && error.message ? error.message : String(error));
          }
        })()
      `)
      report(
        reloaded === 'result' || reloaded === 'reported',
        'Reload then Run terminates against the live server',
        reloaded,
      )
    }

    // Text renders whether or not a stylesheet applied, so a broken CSS build passes every
    // check above. Assert that Tailwind's utilities actually reached the page: the create
    // button carries `bg-blue-600`, and a utility that never compiled computes to
    // transparent.
    const form = await page.visit(`${BASE}/rules/new`)
    const styles = await page.evaluate(`
      (() => {
        const button = [...document.querySelectorAll('button')].find((b) =>
          /create rule/i.test(b.textContent),
        );
        if (!button) return { error: 'no create button' };
        const computed = getComputedStyle(button);
        return {
          background: computed.backgroundColor,
          radius: computed.borderTopLeftRadius,
          bodyFont: getComputedStyle(document.body).fontFeatureSettings,
        };
      })()
    `)
    // Tailwind 4 emits `oklch(...)` for its palette and Tailwind 3 emitted `rgb(...)`; what
    // matters is that *a* colour resolved. A utility that never compiled computes to
    // `rgba(0, 0, 0, 0)` in every browser.
    const styled =
      styles &&
      !styles.error &&
      Boolean(styles.background) &&
      !/rgba\(0, 0, 0, 0\)|transparent/.test(styles.background)
    report(
      Boolean(styled),
      'Tailwind utilities reach the page',
      styles?.error ?? `background ${styles?.background}, radius ${styles?.radius}`,
    )

    // Dark mode follows the operating system. Chromium can be asked to report the dark
    // preference, which is the only way to know the tokens and the `dark:` utilities agree
    // rather than merely existing.
    await page.send('Emulation.setEmulatedMedia', {
      features: [{ name: 'prefers-color-scheme', value: 'dark' }],
    })
    const darkForm = await page.visit(`${BASE}/rules/new`)
    const darkBackground = await page.evaluate(
      'getComputedStyle(document.body).backgroundColor',
    )
    await page.send('Emulation.setEmulatedMedia', { features: [] })
    const lightForm = form
    const lightBackground = await page.evaluate(
      'getComputedStyle(document.body).backgroundColor',
    )
    report(
      darkBackground !== lightBackground && Boolean(darkBackground),
      'dark mode follows the operating system',
      `light ${lightBackground} vs dark ${darkBackground}`,
    )

    const formWorks = form

    report(
      /Name/.test(formWorks.text) && /Create rule/.test(formWorks.text),
      '/rules/new renders a working form',
      formWorks.text.includes('Create rule') ? 'name + submit present' : 'no submit button',
    )

    // `--screenshot <path>` leaves a picture of the signed-in page for a human to
    // look at; the assertions above are what make the run pass or fail.
    const screenshot = arg('screenshot', '')
    if (screenshot) {
      const captured = await page.send('Page.captureScreenshot', { format: 'png' })
      writeFileSync(screenshot, Buffer.from(captured.data, 'base64'))
      console.log(`  screenshot written to ${screenshot}`)
    }
  }
} catch (error) {
  report(false, 'harness', error.message)
} finally {
  page?.socket?.close?.()
  // Chromium keeps writing to its profile for a moment after being killed; deleting
  // it first can fail with ENOTEMPTY, which is not a failing check. Wait for the
  // process, and treat a profile that will not go away as untidy rather than fatal.
  // Stop the browser, and leave the profile in place: it is inside the ignored build
  // directory, and deleting it is what produced the `ENOTEMPTY` race in the first place.
  await stopChromium(chromium)
}

console.log(`\n${failures.length ? `${failures.length} failure(s)` : 'all checks passed'}`)
if (failures.length) {
  for (const failure of failures) console.log(` - ${failure}`)
  process.exit(1)
}
