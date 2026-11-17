// Sandbox side panel: examples and sharing. The workspace does the rest.
import { el, mount } from '../dom.js';
import { icon } from '../icons.js';
import { encodeSource } from '../share.js';

export const SANDBOX_EXAMPLES = [
  {
    id: 'count',
    code: `; Zählen: gibt 1 bis 10 aus.
        mov r1, 1
schleife:
        mov r0, r1
        sys WRITE_INT
        add r1, 1
        cmp r1, 10
        jle schleife
        halt
`,
  },
  {
    id: 'echo',
    code: `; Verdoppeln: liest Zahlen (Konsole) und gibt das Doppelte aus, bis 0 kommt.
schleife:
        sys READ_INT
        cmp r0, 0
        jz fertig
        add r0, r0
        sys WRITE_INT
        jmp schleife
fertig:
        halt
`,
  },
  {
    id: 'hello',
    code: `; Text: ein String im data-Segment, ausgegeben mit WRITE_STR.
        .data
gruss:  .string "Hallo, Stapelwerk\\n"
        .text
        li r0, gruss
        sys WRITE_STR
        halt
`,
  },
  {
    id: 'list',
    code: `; Liste: drei Knoten auf dem Heap, verkettet, ausgegeben, freigegeben.
        mov r4, 0           ; Kopf = NULL
        mov r5, 3
neu:
        mov r0, 8
        sys ALLOC           ; r0 = neuer Knoten
        st r4, [r0]         ; next = Kopf
        st r5, [r0 + 4]     ; wert
        mov r4, r0          ; Kopf = Knoten
        sub r5, 1
        jnz neu
ausgeben:
        cmp r4, 0
        jz fertig
        ld r0, [r4 + 4]
        sys WRITE_INT
        mov r0, r4
        ld r4, [r4]         ; erst weiter, dann freigeben
        sys FREE
        jmp ausgeben
fertig:
        halt
`,
  },
  {
    id: 'flood',
    code: `; Flutfüllung: füllt rekursiv ab (32|32). Die Fläche ist zu groß,
; jeder Aufruf kostet Stack, bis er überläuft.
        mov r0, 32
        mov r1, 32
        call fuell
        halt
fuell:                      ; r0 = x, r1 = y
        cmp r0, 64
        jae .zurueck        ; außerhalb (vorzeichenlos fängt auch -1)
        cmp r1, 64
        jae .zurueck
        mul r2, r1, 64
        add r2, r0
        li r3, SCREEN
        add r2, r3
        ldb r3, [r2]
        cmp r3, 0
        jnz .zurueck        ; schon gefüllt
        mov r3, LIGHTBLUE
        stb r3, [r2]
        push r0
        push r1
        add r0, 1
        call fuell
        ld r0, [sp + 4]
        ld r1, [sp]
        sub r0, 1
        call fuell
        ld r0, [sp + 4]
        ld r1, [sp]
        add r1, 1
        call fuell
        ld r0, [sp + 4]
        ld r1, [sp]
        sub r1, 1
        call fuell
        pop r1
        pop r0
.zurueck:
        ret
`,
  },
];

// Setzwerk examples (#/setzwerk), with comments in both languages.
export const SETZ_EXAMPLES = [
  {
    id: 'hello',
    code: {
      de: `// Hallo: putc gibt ein Zeichen aus, print eine Zahl.
fn main() {
    putc('H'); putc('i'); putc('\\n');
    print(6 * 7);
}
`,
      en: `// Hello: putc outputs a character, print a number.
fn main() {
    putc('H'); putc('i'); putc('\\n');
    print(6 * 7);
}
`,
    },
  },
  {
    id: 'fak',
    code: {
      de: `// Fakultät, rekursiv: jeder Aufruf bekommt einen eigenen Rahmen.
fn fak(n) {
    if (n <= 1) { return 1; }
    return n * fak(n - 1);
}

fn main() {
    print(fak(5));
}
`,
      en: `// Factorial, recursively: every call gets a frame of its own.
fn fak(n) {
    if (n <= 1) { return 1; }
    return n * fak(n - 1);
}

fn main() {
    print(fak(5));
}
`,
    },
  },
  {
    id: 'collatz',
    code: {
      de: `// Collatz: liest Zahlen in der Konsole, bis 0 kommt, und zählt die Schritte bis 1.
fn main() {
    var n = read();
    while (n != 0) {
        var schritte = 0;
        while (n != 1) {
            if (n % 2 == 0) { n = n / 2; } else { n = 3 * n + 1; }
            schritte = schritte + 1;
        }
        print(schritte);
        n = read();
    }
}
`,
      en: `// Collatz: reads numbers in the console until 0 and counts the steps down to 1.
fn main() {
    var n = read();
    while (n != 0) {
        var steps = 0;
        while (n != 1) {
            if (n % 2 == 0) { n = n / 2; } else { n = 3 * n + 1; }
            steps = steps + 1;
        }
        print(steps);
        n = read();
    }
}
`,
    },
  },
  {
    id: 'args',
    code: {
      de: `// Sechs Argumente: vier kommen in r0 bis r3, der Rest über den Stack.
fn summe(a, b, c, d, e, f) {
    return a + b + c + d + e + f;
}

fn main() {
    print(summe(1, 2, 3, 4, 5, 6));
}
`,
      en: `// Six arguments: four travel in r0 to r3, the rest over the stack.
fn sum(a, b, c, d, e, f) {
    return a + b + c + d + e + f;
}

fn main() {
    print(sum(1, 2, 3, 4, 5, 6));
}
`,
    },
  },
];

/**
 * Side panel of a sandbox. `examples` are [{id, code}] with `code` a string or {de, en};
 * `texts` is the i18n key prefix ('sandbox' or 'setz'); `route` the share route.
 */
export function createSandboxPanel({ t, lang = 'de', getCode, onLoad, examples = SANDBOX_EXAMPLES, texts = 'sandbox', route = 'sandbox' }) {
  const shareOut = el('input', { class: 'mono share-out', type: 'text', readonly: true, 'aria-label': t('sandbox.shareLink'), hidden: true });
  const shareNote = el('p', { class: 'muted share-note', 'aria-live': 'polite' });
  const node = el('div', { class: 'sandbox-panel prose' },
    el('h2', {}, t(`${texts}.title`)),
    el('p', {}, t(`${texts}.intro`)),
    el('h3', {}, t('sandbox.examples')),
    el('ul', { class: 'examples' }, examples.map((ex) => el('li', {},
      el('button', { type: 'button', class: 'example', onclick: () => onLoad(typeof ex.code === 'string' ? ex.code : ex.code[lang] ?? ex.code.de) },
        el('strong', {}, t(`${texts}.ex.${ex.id}`)),
        el('span', { class: 'muted' }, t(`${texts}.ex.${ex.id}.note`)))))),
    el('h3', {}, t('sandbox.share')),
    el('p', { class: 'muted' }, t('sandbox.shareNote')),
    el('button', {
      type: 'button',
      class: 'btn',
      onclick: async () => {
        const code = await encodeSource(getCode());
        const url = new URL(location.href);
        url.hash = `#/${route}?code=${code}`;
        shareOut.hidden = false;
        shareOut.value = url.href;
        shareOut.select();
        try { await navigator.clipboard.writeText(url.href); mount(shareNote, t('sandbox.copied')); } catch { mount(shareNote, t('sandbox.copyManual')); }
      },
    }, icon('link'), t('sandbox.shareButton')),
    shareOut, shareNote);
  return { node };
}
