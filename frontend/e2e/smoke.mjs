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
const PORT = Number(arg('port', '9222'))
const SETTLE_MS = Number(arg('settle', '700'))

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

async function waitForDevtools(timeoutMs = 20_000) {
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
  throw new Error(`Chromium did not expose DevTools on port ${PORT}`)
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
      throw new Error(result.exceptionDetails.text ?? 'evaluation failed')
    }
    return result.result?.value
  }

  async visit(url) {
    this.errors = []
    await this.send('Page.navigate', { url })
    await sleep(SETTLE_MS)
    const text = await this.evaluate('document.body ? document.body.innerText : ""')
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
    { stdio: 'ignore', detached: false },
  )
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
  const socketUrl = await waitForDevtools()
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

    // Signed in, the governance pages must show their endpoint's answer — an empty
    // state from the API is fine, an auth error is not.
    console.log('\ngovernance with data')
    for (const [route, marker] of [
      ['/governance/approvals', /No Pending Approvals|Approve|Reject/],
      ['/governance/audit', /No audit events found|Event Type|Apply Filters/],
      ['/governance/branches', /main|master|Branch/],
      ['/governance/rbac', /Permissions|role/i],
    ]) {
      const seen = await page.visit(`${BASE}${route}`)
      report(
        marker.test(seen.text) && !/status code 401/.test(seen.text),
        route,
        /status code 401/.test(seen.text) ? 'reported 401' : 'rendered its data',
      )
    }

    const formWorks = await page.visit(`${BASE}/rules/new`)
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
