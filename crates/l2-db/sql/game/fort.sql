CREATE TABLE IF NOT EXISTS `fort` (
  `id` int(11) NOT NULL DEFAULT 0,
  `name` varchar(25) NOT NULL,
  `siegeDate` bigint(13) unsigned NOT NULL DEFAULT '0',
  `lastOwnedTime` bigint(13) unsigned NOT NULL DEFAULT '0',
  `owner` int(11) NOT NULL DEFAULT 0,
  `fortType` int(1) NOT NULL DEFAULT 0,
  `state` int(1) NOT NULL DEFAULT 0,
  `castleId` int(1) NOT NULL DEFAULT 0,
  `supplyLvL` int(2) NOT NULL DEFAULT 0,
  PRIMARY KEY (`id`),
  KEY `owner` (`owner`)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;