CREATE TABLE IF NOT EXISTS `global_variables` (
  `var`  VARCHAR(191) NOT NULL DEFAULT '',
  `value` VARCHAR(255) ,
  PRIMARY KEY (`var`)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
REPLACE INTO `global_variables` VALUES ('HBLevel', '11');
REPLACE INTO `global_variables` VALUES ('HBTrust', '4000000');
