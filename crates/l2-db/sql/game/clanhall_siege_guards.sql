CREATE TABLE IF NOT EXISTS `clanhall_siege_guards` (
  `clanHallId` tinyint(2) unsigned NOT NULL DEFAULT '0',
  `npcId` smallint(5) unsigned NOT NULL DEFAULT '0',
  `x` mediumint(6) NOT NULL DEFAULT '0',
  `y` mediumint(6) NOT NULL DEFAULT '0',
  `z` mediumint(6) NOT NULL DEFAULT '0',
  `heading` mediumint(6) NOT NULL DEFAULT '0',
  `respawnDelay` mediumint(5) NOT NULL DEFAULT '0',
  `isSiegeBoss` enum('false','true') NOT NULL DEFAULT 'false',
  KEY `clanHallId` (`clanHallId`)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
