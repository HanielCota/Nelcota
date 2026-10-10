CREATE TABLE public.notes (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner uuid NOT NULL DEFAULT auth.uid(),
    body text NOT NULL,
    extra jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE public.notes ENABLE ROW LEVEL SECURITY;
CREATE POLICY notes_owner ON public.notes FOR ALL TO authenticated
    USING (owner = auth.uid()) WITH CHECK (owner = auth.uid());
GRANT SELECT, INSERT, UPDATE, DELETE ON public.notes TO authenticated;

INSERT INTO storage.buckets (id) VALUES ('sdk-files') ON CONFLICT DO NOTHING;
CREATE POLICY sdk_files_owner ON storage.objects FOR ALL TO authenticated
    USING (bucket_id = 'sdk-files' AND (storage.foldername(name))[1] = auth.uid()::text)
    WITH CHECK (bucket_id = 'sdk-files' AND (storage.foldername(name))[1] = auth.uid()::text);
