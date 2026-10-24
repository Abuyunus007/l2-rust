CREATE TABLE IF NOT EXISTS `castle` (
  `id` INT NOT NULL DEFAULT 0,
  `name` varchar(25) NOT NULL,
  `taxPercent` INT NOT NULL DEFAULT 15,
  `treasury` BIGINT NOT NULL DEFAULT 0,
  `siegeDate` bigint(13) unsigned NOT NULL DEFAULT '0',
  `regTimeOver` enum('true','false') DEFAULT 'true' NOT NULL,
  `regTimeEnd` bigint(13) unsigned NOT NULL DEFAULT '0',
  `showNpcCrest` enum('true','false') DEFAULT 'false' NOT NULL,
  `ticketBuyCount` smallint(3) NOT NULL DEFAULT 0,
  PRIMARY KEY (`name`),
  KEY `id` (`id`)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;