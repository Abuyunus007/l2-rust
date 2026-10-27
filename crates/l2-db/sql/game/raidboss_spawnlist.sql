CREATE TABLE IF NOT EXISTS `raidboss_spawnlist` (
  `boss_id` smallint(5) unsigned NOT NULL,
  `amount` tinyint(1) unsigned NOT NULL DEFAULT '1',
  `loc_x` mediumint(6) NOT NULL,
  `loc_y` mediumint(6) NOT NULL,
  `loc_z` mediumint(6) NOT NULL,
  `heading` mediumint(6) NOT NULL DEFAULT '0',
  `respawn_delay` mediumint(6) unsigned NOT NULL DEFAULT '86400', -- 24 Hours
  `respawn_random` mediumint(6) unsigned NOT NULL DEFAULT '43200', -- (12 Hours)
  `respawn_time` bigint(13) unsigned NOT NULL DEFAULT '0',
  `currentHp` decimal(8,0) DEFAULT NULL,
  `currentMp` decimal(8,0) DEFAULT NULL,
  PRIMARY KEY (`boss_id`)
) DEFAULT CHARSET=latin1 COLLATE=latin1_general_ci;
