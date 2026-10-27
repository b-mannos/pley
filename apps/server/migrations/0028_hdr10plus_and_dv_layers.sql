-- Probes now read the first frames of PQ video for HDR10+ (it shows only in
-- the frames: such files were taken for plain HDR10), and of Dolby Vision
-- profile 7 for what its enhancement layer holds (full or minimal). The
-- next scan probes these files again to find them.
UPDATE media_files SET probed = 0
WHERE probed = 1 AND media_info IS NOT NULL
  AND EXISTS (
    SELECT 1 FROM json_each(media_info, '$.streams') s
    WHERE json_extract(s.value, '$.kind') = 'video'
      AND (json_extract(s.value, '$.dynamic_range') = 'hdr10'
           OR (json_extract(s.value, '$.dynamic_range') = 'dolby_vision'
               AND json_extract(s.value, '$.dv_profile') = 7))
  );
