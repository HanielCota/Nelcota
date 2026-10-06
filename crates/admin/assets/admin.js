// Painel nelcota: confirmações, busca de tabelas e editor SQL. Sem dependências.
document.addEventListener('submit', (e) => {
  const msg = e.target.dataset.confirm;
  if (msg && !confirm(msg)) e.preventDefault();
});

const el = (tag, text, cls) => {
  const n = document.createElement(tag);
  if (text !== undefined) n.textContent = text;
  if (cls) n.className = cls;
  return n;
};

// Filtro da lista de tabelas do editor.
const search = document.getElementById('table-search');
if (search) {
  const items = [...document.querySelectorAll('#table-list li')];
  search.addEventListener('input', () => {
    const q = search.value.trim().toLowerCase();
    items.forEach((li) => { li.hidden = q !== '' && !li.dataset.name.toLowerCase().includes(q); });
  });
}

// Editor SQL.
const form = document.getElementById('sql-form');
if (form) {
  const input = document.getElementById('sql');
  const out = document.getElementById('sql-result');
  const button = form.querySelector('button');
  const run = async () => {
    button.disabled = true;
    out.replaceChildren(el('div', 'Executando…', 'meta'));
    const started = performance.now();
    try {
      const res = await fetch('/admin/sql', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sql: input.value }) });
      if (!res.ok) { out.replaceChildren(el('div', await res.text(), 'error')); return; }
      const data = await res.json();
      const ms = Math.round(performance.now() - started);
      out.replaceChildren();
      if (data.error) {
        const e = data.error;
        const box = el('div', undefined, 'error');
        box.append(el('strong', `${e.code ? e.code + ' · ' : ''}${e.message}`));
        if (e.position) box.append(el('div', `posição ${e.position}`));
        if (e.detail) box.append(el('div', e.detail));
        if (e.hint) box.append(el('div', 'Dica: ' + e.hint));
        out.append(box);
        return;
      }
      if (!data.results.length) out.append(el('div', `Sem resultado · ${ms} ms`, 'meta'));
      for (const r of data.results) {
        out.append(el('div', `${r.count} linha(s)${r.truncated ? ' · mostrando 1000' : ''} · ${ms} ms`, 'meta'));
        if (!r.columns.length) continue;
        const table = el('table'), thead = el('thead'), head = el('tr'), tbody = el('tbody');
        r.columns.forEach((c) => head.append(el('th', c)));
        thead.append(head);
        for (const row of r.rows) {
          const tr = el('tr');
          row.forEach((v) => tr.append(v === null ? el('td', 'NULL', 'null') : el('td', v)));
          tbody.append(tr);
        }
        table.append(thead, tbody);
        out.append(table);
      }
    } finally {
      button.disabled = false;
    }
  };
  form.addEventListener('submit', (e) => { e.preventDefault(); run(); });
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); run(); }
    if (e.key === 'Tab') { // Tab insere indentação em vez de sair do campo.
      e.preventDefault();
      const { selectionStart: s, selectionEnd: f } = input;
      input.setRangeText('  ', s, f, 'end');
    }
  });
}
