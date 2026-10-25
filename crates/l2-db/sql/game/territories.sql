CREATE TABLE IF NOT EXISTS `territories` (
  `territoryId` INT NOT NULL DEFAULT 0,
  `castleId` INT NOT NULL DEFAULT 0,
  `fortId` INT NOT NULL DEFAULT 0,
  `ownedWardIds` varchar(30) NOT NULL DEFAULT '',
  PRIMARY KEY (`territoryId`)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
