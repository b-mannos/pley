-- Probes now find MP4 and MOV videos whose frames carry the times they are
-- decoded at in place of the times they are shown (B-frames muxed from a
-- raw stream, without composition offsets). Played as they are, such videos
-- stutter; they are converted instead. The next scan probes these files
-- again to find them.
UPDATE media_files SET probed = 0
WHERE probed = 1 AND media_info IS NOT NULL
  AND container IN ('mp4', 'm4v', 'mov') AND video_codec IN ('h264', 'hevc');
