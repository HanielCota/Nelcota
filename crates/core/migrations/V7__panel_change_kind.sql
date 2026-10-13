-- Kind and target of each schema change recorded by the panel (see V6), so
-- the panel can describe a change in the viewer's language instead of
-- showing the stored English summary.

ALTER TABLE nelcota.panel_changes
    ADD COLUMN kind text CHECK (kind IN (
        'table_created', 'table_altered', 'table_dropped',
        'policy_created', 'policy_updated', 'policy_dropped'
    )),
    ADD COLUMN target text;

-- Rows recorded before these columns existed: recover them from the summary,
-- which was written in Portuguese until the codebase moved to English.
UPDATE nelcota.panel_changes AS c
   SET kind = p.kind,
       target = (regexp_match(c.summary, p.pattern))[1]
  FROM (VALUES
        ('table_created',  '^(?:tabela|table) ''(.+)'' (?:criada|created)$'),
        ('table_altered',  '^(?:tabela|table) ''(.+)'' (?:alterada|altered)$'),
        ('table_dropped',  '^(?:tabela|table) ''(.+)'' (?:apagada|dropped)$'),
        ('policy_created', '^policy ''(.+)'' (?:criada|created)$'),
        ('policy_updated', '^policy ''(.+)'' (?:atualizada|updated)$'),
        ('policy_dropped', '^policy ''(.+)'' (?:apagada|dropped)$')
       ) AS p(kind, pattern)
 WHERE c.kind IS NULL
   AND c.summary ~ p.pattern;
