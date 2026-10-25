CREATE TABLE IF NOT EXISTS `territory_spawnlist` (
  `castleId` tinyint(1) unsigned NOT NULL DEFAULT '0',
  `id` smallint(3) unsigned NOT NULL AUTO_INCREMENT,
  `npcId` smallint(5) unsigned NOT NULL DEFAULT '0',
  `x` mediumint(6) NOT NULL DEFAULT '0',
  `y` mediumint(6) NOT NULL DEFAULT '0',
  `z` mediumint(6) NOT NULL DEFAULT '0',
  `heading` mediumint(6) NOT NULL DEFAULT '0',
  `spawnType` tinyint(1) unsigned NOT NULL DEFAULT '0', -- 0-spawned when castle have owner, 1-fortress spawns in TW time, 2-Castle spawns in TW time, 3-Ward spawn places
  PRIMARY KEY (`id`),
  KEY `id` (`castleId`)
) DEFAULT CHARSET=latin1 COLLATE=latin1_general_ci;
