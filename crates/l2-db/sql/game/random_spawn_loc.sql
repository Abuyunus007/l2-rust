CREATE TABLE IF NOT EXISTS `random_spawn_loc` (
  `groupId` tinyint(3) unsigned NOT NULL,
  `x` mediumint(6) NOT NULL,
  `y` mediumint(6) NOT NULL,
  `z` mediumint(6) NOT NULL,
  `heading` mediumint(6) NOT NULL DEFAULT '0',
  PRIMARY KEY (`groupId`,`x`,`y`,`z`,`heading`)
) DEFAULT CHARSET=latin1 COLLATE=latin1_general_ci;
