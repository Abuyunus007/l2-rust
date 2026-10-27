CREATE TABLE IF NOT EXISTS `seven_signs_festival` (
  `festivalId` int(1) NOT NULL DEFAULT '0',
  `cabal` varchar(4) NOT NULL DEFAULT '',
  `cycle` int(4) NOT NULL DEFAULT '0',
  `date` bigint(13) unsigned NOT NULL DEFAULT '0',
  `score` int(5) NOT NULL DEFAULT '0',
  `members` varchar(255) NOT NULL DEFAULT '',
  PRIMARY KEY (`festivalId`,`cabal`,`cycle`)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
