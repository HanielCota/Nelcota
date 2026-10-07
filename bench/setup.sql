-- Benchmark data: 100k products, readable by anon (no RLS, to measure the API
-- rather than the policy).
DROP TABLE IF EXISTS public.bench_products;
CREATE TABLE public.bench_products (
    id         integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name       text          NOT NULL,
    price      numeric(10,2) NOT NULL,
    stock      integer       NOT NULL,
    created_at timestamptz   NOT NULL DEFAULT now()
);
INSERT INTO public.bench_products (name, price, stock)
SELECT 'product ' || i, (i % 500) + 0.99, i % 50 FROM generate_series(1, 100000) i;
CREATE INDEX ON public.bench_products (stock, id);
GRANT SELECT ON public.bench_products TO anon;
ANALYZE public.bench_products;
