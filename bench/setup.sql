-- Dados do benchmark: 100 mil produtos, leitura liberada para anon (sem RLS,
-- para medir a API e não a policy; ver leitura-rls no README).
DROP TABLE IF EXISTS public.bench_produtos;
CREATE TABLE public.bench_produtos (
    id        integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    nome      text          NOT NULL,
    preco     numeric(10,2) NOT NULL,
    estoque   integer       NOT NULL,
    criado_em timestamptz   NOT NULL DEFAULT now()
);
INSERT INTO public.bench_produtos (nome, preco, estoque)
SELECT 'produto ' || i, (i % 500) + 0.99, i % 50 FROM generate_series(1, 100000) i;
CREATE INDEX ON public.bench_produtos (estoque, id);
GRANT SELECT ON public.bench_produtos TO anon;
ANALYZE public.bench_produtos;
