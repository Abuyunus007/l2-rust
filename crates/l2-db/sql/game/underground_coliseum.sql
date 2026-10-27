CREATE TABLE IF NOT EXISTS `underground_coliseum` (
  `arenaId` int(1) unsigned NOT NULL,
  `leader` varchar(16) NOT NULL,
  `wins` int(5) NOT NULL DEFAULT 0,
  PRIMARY KEY (`arenaId`)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;