// Painel nelcota: confirmação de ações destrutivas e editor SQL. Sem dependências.
document.addEventListener('submit', (e) => {
  const msg = e.target.dataset.confirm;
  if (msg && !confirm(msg)) e.preventDefault();
});

const form = document.getElementById('sql-form');
if (form) {
  const input = document.getElementById('sql');
  const out = document.getElementById('sql-result');
  const el = (tag, text, cls) => { const n = document.createElement(tag); if (text !== undefined) n.textContent = text; if (cls) n.className = cls; return n; };
  const run = async () => {
    out.replaceChildren(el('p', 'Executando…', 'muted'));
    const started = performance.now();
    const res = await fetch('/admin/sql', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sql: input.value }) });
    if (!res.ok) { out.replaceChildren(el('p', await res.text(), 'error')); return; }
    const data = await res.json();
    const ms = Math.round(performance.now() - started);
    out.replaceChildren();
    if (data.error) {
      const e = data.error;
      out.append(el('p', `${e.code ? e.code + ': ' : ''}${e.message}${e.position ? ' (posição ' + e.position + ')' : ''}`, 'error'));
      if (e.detail) out.append(el('p', e.detail, 'muted'));
      if (e.hint) out.append(el('p', 'Dica: ' + e.hint, 'muted'));
      return;
    }
    for (const r of data.results) {
      out.append(el('p', `${r.count} linha(s)${r.truncated ? ' (mostrando 1000)' : ''} · ${ms} ms`, 'muted'));
      if (!r.columns.length) continue;
      const table = el('table'), head = el('tr');
      r.columns.forEach((c) => head.append(el('th', c)));
      table.append(head);
      for (const row of r.rows) {
        const tr = el('tr');
        row.forEach((v) => tr.append(v === null ? el('td', 'NULL', 'null') : el('td', v)));
        table.append(tr);
      }
      const wrap = el('div', undefined, 'scroll'); wrap.append(table); out.append(wrap);
    }
  };
  form.addEventListener('submit', (e) => { e.preventDefault(); run(); });
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); run(); } });
}
