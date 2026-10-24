CREATE TABLE IF NOT EXISTS `siegable_clanhall` (
  `clanHallId` int(10) NOT NULL DEFAULT '0',
  `name` varchar(45) DEFAULT NULL,
  `ownerId` int(10) DEFAULT NULL,
  `desc` varchar(100) DEFAULT NULL,
  `location` varchar(100) DEFAULT NULL,
  `nextSiege` bigint(20) DEFAULT NULL,
  `siegeLength` int(10) DEFAULT NULL,
  `schedule_config` varchar(20) DEFAULT NULL,
  PRIMARY KEY (`clanHallId`),
  KEY `ownerId` (`ownerId`)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;