// Setzwerk: the compiler view (#/setzwerk) in German and English, compile errors at the right
// line, stepping through the generated assembly with the source line marked, and chapter 8
// levels solved with their Setzwerk reference.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { WS_STATE, openApp, referenceOf, skip, startOf, useBrowser } from './harness.mjs';

const ctx = useBrowser();
const T = { skip, timeout: 90000 };
const SRC_CUR = `(() => { const g = document.querySelector('.setz-side:not(.setz-asm) .gl.is-current'); return g ? Number(g.dataset.line) : null; })()`;
const ASM = `document.querySelector('.setz-asm .ed-text').value`;

async function assertClean(app) {
  assert.deepEqual(app.errors, [], 'console errors');
  assert.deepEqual(await app.csp(), [], 'CSP violations');
  assert.deepEqual(app.foreignRequests(), [], 'requests outside the test server');
}

test('Setzwerk view (de): compile error at its line, then compile, step and run', T, async () => {
  const app = await openApp(ctx, '/stapelwerk/#/setzwerk');
  try {
    assert.equal(await app.evaluate(`document.querySelector('.mainnav a[data-route="setzwerk"]')?.getAttribute('aria-current')`), 'page', 'nav entry marked');
    assert.equal(await app.evaluate(`document.querySelectorAll('.setz-side .ed-text').length`), 2);
    assert.equal(await app.evaluate(`document.querySelector('.setz-asm .ed-text').readOnly`), true);
    // A compile error: the notice names the line, the message sits under the source.
    await app.setCode('fn main() {\n    print(x);\n}\n');
    await app.click('.dbg-primary');
    await app.until(`/Zeile 2/.test((${WS_STATE}).notice)`, 'compile error notice');
    assert.match(await app.evaluate(`document.querySelector('.setz-side:not(.setz-asm) .ed-errors').textContent`), /Unbekannte Variable „x“/);
    assert.equal(await app.evaluate(`document.querySelector('.status-chip').dataset.kind`), 'idle');
    // A correct program: the assembly appears with German comments and runs on the machine.
    await app.setCode('fn main() {\n    var a = 6;\n    print(a * 7);\n}\n');
    await app.click('.dbg-primary');
    await app.until(`/Übersetzt/.test((${WS_STATE}).notice)`, 'compiled');
    assert.match(await app.evaluate(ASM), /; Zeile 3: print\(a \* 7\);/);
    assert.match(await app.evaluate(ASM), /; Prolog/);
    assert.equal(await app.evaluate(`document.querySelector('.setz-side:not(.setz-asm) .ed-errors').hidden`), true);
    // Step into main: the source side marks the Setzwerk line of the current instruction.
    await app.press('Escape');
    for (let i = 0; i < 4; i++) await app.press('s');
    await app.until(`${SRC_CUR} === 2`, 'source line 2 marked');
    await app.press('l');
    await app.until(`(${WS_STATE}).kind === 'halted'`, 'halted');
    assert.equal(await app.evaluate(`document.querySelector('.tape-out').textContent.trim()`), '42');
    // The code is kept per view.
    await app.until(`JSON.parse(localStorage.getItem('stapelwerk:progress')).levels.setzwerk?.code?.includes('a * 7')`, 'code saved');
    await assertClean(app);
  } finally {
    await app.close();
  }
});

test('Setzwerk view (en): English comments and messages, breakpoint from the source', T, async () => {
  const app = await openApp(ctx, '/stapelwerk/en/#/setzwerk');
  try {
    assert.equal(await app.evaluate(`document.documentElement.lang`), 'en');
    await app.setCode('fn main() {\n    var i = 0;\n    while (i < 3) {\n        print(i);\n        i = i + 1;\n    }\n}\n');
    await app.click('.dbg-primary');
    await app.until(`/Compiled/.test((${WS_STATE}).notice)`, 'compiled');
    assert.match(await app.evaluate(ASM), /; line 4: print\(i\);/);
    assert.doesNotMatch(await app.evaluate(ASM), /Zeile|Prolog/);
    // Breakpoint on Setzwerk line 4 (gutter click), run: stops there with the source line marked.
    await app.click('.setz-side:not(.setz-asm) .gl[data-line="4"]');
    await app.until(`!!document.querySelector('.setz-side:not(.setz-asm) .gl.has-bp[data-line="4"]')`, 'breakpoint shown in the source');
    await app.press('Escape');
    await app.press('l');
    await app.until(`(${WS_STATE}).kind === 'breakpoint' && ${SRC_CUR} === 4`, 'stopped at line 4');
    await app.setCode('fn main() {\n    print(1)\n}\n');
    await app.click('.dbg-primary');
    await app.until(`/line 3/.test((${WS_STATE}).notice)`, 'error notice');
    assert.match(await app.evaluate(`document.querySelector('.setz-side:not(.setz-asm) .ed-errors').textContent`), /';' is missing here, found '}'/);
    await assertClean(app);
  } finally {
    await app.close();
  }
});

test('chapter 8: the start code fails, the Setzwerk references solve 8.1 and 8.2', T, async () => {
  const app = await openApp(ctx, '/stapelwerk/#/level/8-1-quersumme');
  try {
    assert.equal((await app.evaluate(`document.querySelector('.setz-side .ed-text').value`)).trimEnd(), startOf('8-1-quersumme.lvl').trimEnd());
    await app.click('.dbg-primary');
    await app.until(`/Tests/.test((${WS_STATE}).notice) || /fehlgeschlagen|nicht bestanden/i.test((${WS_STATE}).notice)`, 'start code fails');
    assert.equal((await app.evaluate(WS_STATE)).result, false);
    await app.setCode(referenceOf('8-1-quersumme.lvl'));
    await app.click('.dbg-primary');
    await app.until(`(${WS_STATE}).result`, 'result view 8.1');
    assert.match((await app.evaluate(WS_STATE)).notice, /Gelöst/);
    await app.goto('/stapelwerk/#/level/8-2-ggt');
    await app.setCode(referenceOf('8-2-ggt.lvl'));
    await app.click('.dbg-primary');
    await app.until(`(${WS_STATE}).result`, 'result view 8.2 (function test, ABI checked)');
    await app.goto('/stapelwerk/#/');
    await app.until(`document.querySelectorAll('.station').length === 35`, 'plan with chapter 8');
    assert.match(await app.evaluate(`document.querySelector('#ch-8').textContent`), /Setzwerk/);
    await assertClean(app);
  } finally {
    await app.close();
  }
});
