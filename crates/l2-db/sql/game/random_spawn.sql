CREATE TABLE IF NOT EXISTS `random_spawn` (
  `groupId` tinyint(3) unsigned NOT NULL,
  `npcId` smallint(5) unsigned NOT NULL,
  `count` tinyint(1) unsigned NOT NULL DEFAULT '1',
  `initialDelay` int(8) NOT NULL DEFAULT '-1',
  `respawnDelay` int(8) NOT NULL DEFAULT '-1',
  `despawnDelay` int(8) NOT NULL DEFAULT '-1',
  `broadcastSpawn` enum('true','false') NOT NULL DEFAULT 'false',
  `randomSpawn` enum('true','false') NOT NULL DEFAULT 'true',
  PRIMARY KEY (`groupId`)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
