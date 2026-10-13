-- Each seek finds one visible folder, then skips its entire subtree. The
-- directory range and every seek still run under the caller's objects RLS.
WITH RECURSIVE folders AS (
    (SELECT split_part(substr(o.name, length($2::text) + 1), '/', 1) AS folder,
            $3::bytea || convert_to(split_part(substr(o.name, length($2) + 1), '/', 1), 'UTF8')
                || decode('01', 'hex') AS next_key,
            1::bigint AS position
       FROM storage.objects o
      WHERE o.bucket_id = $1 AND o.directory_key > $3
        AND ($4::bytea IS NULL OR o.directory_key < $4)
      ORDER BY o.directory_key LIMIT 1)
    UNION ALL
    SELECT next.folder, next.next_key, f.position + 1
      FROM folders f CROSS JOIN LATERAL (
        SELECT split_part(substr(o.name, length($2) + 1), '/', 1) AS folder,
               $3 || convert_to(split_part(substr(o.name, length($2) + 1), '/', 1), 'UTF8')
                   || decode('01', 'hex') AS next_key
          FROM storage.objects o
         WHERE o.bucket_id = $1 AND o.directory_key >= f.next_key
           AND ($4 IS NULL OR o.directory_key < $4)
         ORDER BY o.directory_key LIMIT 1
      ) next
     WHERE f.position - $6::bigint < $5::bigint
)
SELECT json_build_object(
    'folders', coalesce((
        SELECT json_agg(folder ORDER BY folder) FROM folders WHERE position > $6
    ), '[]'),
    'objects', coalesce((
        SELECT json_agg(json_build_object(
            'id', id, 'name', name, 'size', size, 'mime_type', mime_type,
            'etag', etag, 'owner', owner, 'metadata', metadata,
            'created_at', created_at, 'updated_at', updated_at) ORDER BY name)
          FROM (SELECT * FROM storage.objects
                 WHERE directory_scope = storage.directory_hash($1, $3)
                   -- Verify the fingerprint's scope without selecting an
                   -- unordered parent scan when its column has no statistics.
                   AND bucket_id IS NOT DISTINCT FROM $1
                   AND directory_key IS NOT DISTINCT FROM $3
                   AND name >= $2
                   AND ($2 = '' OR name < substr($2, 1, length($2) - 1) || '0')
                 ORDER BY name LIMIT $5 OFFSET $6) page
    ), '[]')
)::text
