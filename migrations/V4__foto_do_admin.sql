-- Foto de perfil do admin do painel.
--
-- Fica no schema `nelcota`, interno do Nelcota (é onde já vive o histórico de
-- migrações): nenhuma role da API (anon, authenticated, service_role) tem
-- USAGE nele, então a tabela não aparece no REST nem pode ser lida por SQL de
-- quem não é o dono do banco.
--
-- Uma linha por email de admin. A imagem já chega recortada e reduzida pelo
-- painel; o limite de tamanho aqui é a última barreira.

CREATE SCHEMA IF NOT EXISTS nelcota;

CREATE TABLE IF NOT EXISTS nelcota.admin_avatar (
    email        text PRIMARY KEY CHECK (email = lower(email)),
    content_type text NOT NULL CHECK (content_type IN ('image/png', 'image/jpeg', 'image/webp')),
    image        bytea NOT NULL CHECK (octet_length(image) BETWEEN 1 AND 262144),
    updated_at   timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE nelcota.admin_avatar IS 'Foto de perfil do admin do painel (/admin).';
