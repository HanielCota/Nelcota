-- Inline both uses: folders need only names, objects need only a page of metadata.
WITH under AS NOT MATERIALIZED (
    SELECT o.*, substr(o.name, $3 + 1) AS rest
      FROM storage.objects o
     WHERE o.bucket_id = $1 AND o.name LIKE $2 ESCAPE '\'
)
SELECT json_build_object(
    'folders', coalesce((
        SELECT json_agg(f ORDER BY f) FROM (
            SELECT DISTINCT split_part(rest, '/', 1) AS f
              FROM under WHERE strpos(rest, '/') > 0
             ORDER BY 1 LIMIT $4 OFFSET $5
        ) d), '[]'),
    'objects', coalesce((
        SELECT json_agg(json_build_object(
            'id', id, 'name', name, 'size', size, 'mime_type', mime_type,
            'etag', etag, 'owner', owner, 'metadata', metadata,
            'created_at', created_at, 'updated_at', updated_at) ORDER BY name)
          FROM (SELECT * FROM under WHERE strpos(rest, '/') = 0
                 ORDER BY name LIMIT $4 OFFSET $5) page), '[]')
)::text
