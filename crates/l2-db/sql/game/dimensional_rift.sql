CREATE TABLE IF NOT EXISTS `dimensional_rift` (
  `type` tinyint(1) unsigned NOT NULL,
  `room_id` tinyint(1) unsigned NOT NULL,
  `xMin` mediumint(6) NOT NULL,
  `xMax` mediumint(6) NOT NULL,
  `yMin` mediumint(6) NOT NULL,
  `yMax` mediumint(6) NOT NULL,
  `zMin` mediumint(6) NOT NULL,
  `zMax` mediumint(6) NOT NULL,
  `xT` mediumint(6) NOT NULL,
  `yT` mediumint(6) NOT NULL,
  `zT` mediumint(6) NOT NULL,
  `boss` tinyint(1) unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`type`,`room_id`)
) DEFAULT CHARSET=latin1 COLLATE=latin1_general_ci;
