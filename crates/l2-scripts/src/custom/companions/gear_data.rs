//! Generated from the item and armor set data of the datapack (`data/stats/items`, `armorsets`):
//! the weapons and armor a companion wears, by grade. Item ids only; the test checks each one
//! against the datapack.

/// `(role key, grade index, weapon item id)`. The grades are D, C, B, A, S, S80, S84.
pub const WEAPONS: &[(&str, usize, i32)] = &[
    ("tank", 0, 2499),      // Elven Long Sword
    ("tank", 0, 129),       // Sword of Revolution
    ("tank", 0, 7886),      // Sword of Magic Fog
    ("tank", 0, 127),       // Crimson Sword
    ("tank", 1, 5800),      // Nephilim Lord
    ("tank", 1, 135),       // Samurai Longsword
    ("tank", 1, 7888),      // Ecliptic Sword
    ("tank", 1, 75),        // Caliburs
    ("tank", 2, 79),        // Sword of Damascus
    ("tank", 2, 146),       // Ghoulbane
    ("tank", 2, 142),       // Keshanberk
    ("tank", 2, 148),       // Sword of Valhalla
    ("tank", 3, 8678),      // Sirra's Blade
    ("tank", 3, 8686),      // Themis' Tongue
    ("tank", 3, 151),       // Sword of Miracles
    ("tank", 3, 2500),      // Dark Legion's Edge
    ("tank", 4, 9442),      // Dynasty Sword
    ("tank", 4, 9444),      // Dynasty Phantom
    ("tank", 4, 6364),      // Forgotten Blade
    ("tank", 5, 10215),     // Icarus Sawsword
    ("tank", 5, 10217),     // Icarus Spirit
    ("tank", 6, 15544),     // Eternal Core Sword
    ("tank", 6, 15553),     // Archangel Sword
    ("tank", 6, 15558),     // Periel Sword
    ("tank", 6, 15567),     // Veniplant Sword
    ("warrior", 0, 70),     // Claymore
    ("warrior", 0, 7881),   // Titan Sword
    ("warrior", 0, 124),    // Two-Handed Sword
    ("warrior", 0, 5285),   // Heavy Sword
    ("warrior", 1, 5286),   // Berserker Blade
    ("warrior", 1, 7882),   // Pa'agrian Sword
    ("warrior", 1, 71),     // Flamberge
    ("warrior", 2, 7883),   // Guardian Sword
    ("warrior", 2, 78),     // Great Sword
    ("warrior", 3, 8679),   // Sword of Ipos
    ("warrior", 3, 81),     // Dragon Slayer
    ("warrior", 4, 9443),   // Dynasty Blade
    ("warrior", 4, 21959),  // Blood Brother
    ("warrior", 4, 6372),   // Heaven's Divider
    ("warrior", 5, 10218),  // Icarus Heavy Arms
    ("warrior", 6, 15548),  // Lava Saw
    ("warrior", 6, 15562),  // Feather Eye Blade
    ("warrior", 6, 15680),  // Triumph Two Hand Sword
    ("warrior", 6, 13458),  // Vesper Slasher
    ("healer", 0, 159),     // Bonebreaker
    ("healer", 0, 189),     // Staff of Life
    ("healer", 0, 88),      // Morning Star
    ("healer", 0, 158),     // Tarbar
    ("healer", 1, 2503),    // Yaksa Mace
    ("healer", 1, 7891),    // Ecliptic Axe
    ("healer", 1, 162),     // War Axe
    ("healer", 1, 174),     // Nirvana Axe
    ("healer", 2, 7834),    // Art of Battle Axe
    ("healer", 2, 171),     // Deadman's Glory
    ("healer", 2, 175),     // Art of Battle Axe
    ("healer", 2, 91),      // Heavy War Axe
    ("healer", 3, 8680),    // Barakiel's Axe
    ("healer", 3, 8687),    // Cabrio's Hand
    ("healer", 3, 164),     // Elysian
    ("healer", 3, 7895),    // Flaming Dragon Skull
    ("healer", 4, 9448),    // Dynasty Cudgel
    ("healer", 4, 9449),    // Dynasty Mace
    ("healer", 4, 6365),    // Basalt Battlehammer
    ("healer", 4, 6579),    // Arcana Mace
    ("healer", 5, 10220),   // Icarus Hammer
    ("healer", 5, 10222),   // Icarus Hall
    ("healer", 6, 15546),   // Eversor Mace
    ("healer", 6, 15551),   // Sacredium
    ("healer", 6, 15560),   // Vigwik Axe
    ("healer", 6, 15565),   // Rising Star
    ("archer", 0, 280),     // Light Crossbow
    ("archer", 0, 279),     // Reinforced Long Bow
    ("archer", 0, 278),     // Gastraphetes
    ("archer", 0, 275),     // Long Bow
    ("archer", 1, 286),     // Eminence Bow
    ("archer", 1, 283),     // Akat Long Bow
    ("archer", 1, 282),     // Elemental Bow
    ("archer", 1, 285),     // Noble Elven Bow
    ("archer", 2, 287),     // Bow of Peril
    ("archer", 2, 284),     // Dark Elven Long Bow
    ("archer", 3, 8684),    // Shyeed's Bow
    ("archer", 3, 289),     // Soul Bow
    ("archer", 3, 288),     // Carnage Bow
    ("archer", 4, 9445),    // Dynasty Bow
    ("archer", 4, 6368),    // Shining Bow
    ("archer", 4, 7575),    // Draconic Bow
    ("archer", 4, 290),     // The Bow
    ("archer", 5, 10223),   // Icarus Spitter
    ("archer", 6, 15554),   // Recurve Thorne Bow
    ("archer", 6, 15568),   // Skull Carnium Bow
    ("archer", 6, 15686),   // Triumph Bow
    ("archer", 6, 13467),   // Vesper Thrower
    ("dagger", 0, 225),     // Mithril Dagger
    ("dagger", 0, 224),     // Maingauche
    ("dagger", 0, 223),     // Kukuri
    ("dagger", 0, 238),     // Dagger of Mana
    ("dagger", 1, 228),     // Crystal Dagger
    ("dagger", 1, 231),     // Grace Dagger
    ("dagger", 1, 233),     // Dark Screamer
    ("dagger", 1, 227),     // Stiletto
    ("dagger", 2, 234),     // Demon's Dagger
    ("dagger", 2, 229),     // Kris
    ("dagger", 2, 243),     // Hell Knife
    ("dagger", 3, 8682),    // Naga Storm
    ("dagger", 3, 236),     // Soul Separator
    ("dagger", 3, 235),     // Bloody Orchid
    ("dagger", 4, 9446),    // Dynasty Knife
    ("dagger", 4, 6367),    // Angel Slayer
    ("dagger", 4, 237),     // Dragon's Tooth
    ("dagger", 5, 10216),   // Icarus Disperser
    ("dagger", 6, 15545),   // Mamba Edge
    ("dagger", 6, 15559),   // Skull Edge
    ("dagger", 6, 15677),   // Triumph Dagger
    ("dagger", 6, 13460),   // Vesper Shaper
    ("mage", 0, 187),       // Atuba Hammer
    ("mage", 0, 188),       // Ghost Staff
    ("mage", 0, 189),       // Staff of Life
    ("mage", 0, 190),       // Atuba Mace
    ("mage", 1, 204),       // Deadman's Staff
    ("mage", 1, 205),       // Ghoul's Staff
    ("mage", 1, 206),       // Demon's Staff
    ("mage", 1, 7888),      // Ecliptic Sword
    ("mage", 2, 209),       // Divine Staff
    ("mage", 2, 211),       // Staff of Nobility
    ("mage", 2, 210),       // Staff of Evil Spirits
    ("mage", 2, 146),       // Ghoulbane
    ("mage", 3, 8686),      // Themis' Tongue
    ("mage", 3, 8687),      // Cabrio's Hand
    ("mage", 3, 8688),      // Daimon Crystal
    ("mage", 3, 151),       // Sword of Miracles
    ("mage", 4, 9444),      // Dynasty Phantom
    ("mage", 4, 9449),      // Dynasty Mace
    ("mage", 4, 10252),     // Dynasty Staff
    ("mage", 4, 6366),      // Imperial Staff
    ("mage", 5, 10217),     // Icarus Spirit
    ("mage", 5, 10222),     // Icarus Hall
    ("mage", 6, 15551),     // Sacredium
    ("mage", 6, 15552),     // Cyclic Cane
    ("mage", 6, 15553),     // Archangel Sword
    ("mage", 6, 15565),     // Rising Star
    ("dual", 0, 2529),      // Bastard Sword*Crimson Sword
    ("dual", 0, 2530),      // Bastard Sword*Elven Sword
    ("dual", 0, 2536),      // Spinebone Sword*Crimson Sword
    ("dual", 0, 2537),      // Spinebone Sword*Elven Sword
    ("dual", 1, 2572),      // Shamshir*Shamshir
    ("dual", 1, 2573),      // Shamshir*Katana
    ("dual", 1, 2574),      // Shamshir*Spirit Sword
    ("dual", 1, 2575),      // Shamshir*Raid Sword
    ("dual", 2, 2626),      // Samurai Long Sword*Samurai Long Sword
    ("dual", 2, 2611),      // Caliburs*Samurai Long Sword
    ("dual", 2, 2616),      // Sword of Limit*Samurai Long Sword
    ("dual", 2, 2620),      // Sword of Delusion*Samurai Long Sword
    ("dual", 3, 5706),      // Damascus*Damascus
    ("dual", 3, 5705),      // Keshanberk*Damascus
    ("dual", 3, 5704),      // Keshanberk*Keshanberk
    ("dual", 3, 5233),      // Keshanberk*Keshanberk
    ("dual", 4, 10004),     // Dynasty Dual Sword
    ("dual", 4, 21955),     // Blades of Delusion
    ("dual", 4, 6580),      // Tallum Blade*Dark Legion's Edge
    ("dual", 5, 10415),     // Icarus Dual Sword
    ("dual", 6, 16158),     // Eternal Core Dual Sword
    ("dual", 6, 16154),     // Periel Dual Sword
    ("dual", 6, 52),        // Vesper Dual Sword
    ("pole", 0, 297),       // Glaive
    ("pole", 0, 93),        // Winged Spear
    ("pole", 0, 294),       // War Pick
    ("pole", 0, 293),       // War Hammer
    ("pole", 1, 299),       // Orcish Poleaxe
    ("pole", 1, 301),       // Scorpion
    ("pole", 1, 303),       // Widow Maker
    ("pole", 1, 95),        // Poleaxe
    ("pole", 2, 97),        // Lance
    ("pole", 2, 300),       // Great Axe
    ("pole", 3, 8683),      // Tiphon's Spear
    ("pole", 3, 304),       // Orcish Halberd
    ("pole", 3, 5629),      // Orcish Halberd
    ("pole", 3, 5630),      // Orcish Halberd
    ("pole", 4, 9447),      // Dynasty Halberd
    ("pole", 4, 6370),      // Saint Spear
    ("pole", 4, 306),       // Dragon Claw Axe
    ("pole", 5, 10219),     // Icarus Trident
    ("pole", 6, 15550),     // Demitelum
    ("pole", 6, 15564),     // Doubletop Spear
    ("pole", 6, 15682),     // Triumph Spear
    ("pole", 6, 13462),     // Vesper Stormer
    ("fist", 0, 262),       // Scallop Jamadhr
    ("fist", 0, 261),       // Bich'Hwa
    ("fist", 0, 260),       // Triple-Edged Jamadhr
    ("fist", 0, 259),       // Single-Edged Jamadhr
    ("fist", 1, 266),       // Great Pata
    ("fist", 1, 265),       // Fisted Blade
    ("fist", 1, 4233),      // Knuckle Duster
    ("fist", 1, 263),       // Chakram
    ("fist", 2, 268),       // Bellion Cestus
    ("fist", 2, 264),       // Pata
    ("fist", 2, 267),       // Arthro Nail
    ("fist", 3, 8685),      // Sobekk's Hurricane
    ("fist", 3, 270),       // Dragon Grinder
    ("fist", 3, 269),       // Blood Tornado
    ("fist", 4, 9450),      // Dynasty Bagh-Nakh
    ("fist", 4, 6371),      // Demon Splinter
    ("fist", 5, 10221),     // Icarus Hand
    ("fist", 6, 15549),     // Jade Claw
    ("fist", 6, 15563),     // Octo Claw
    ("fist", 6, 15681),     // Triumph Jamadhr
    ("fist", 6, 13461),     // Vesper Fighter
    ("blunt", 0, 159),      // Bonebreaker
    ("blunt", 0, 88),       // Morning Star
    ("blunt", 0, 158),      // Tarbar
    ("blunt", 0, 169),      // Skull Breaker
    ("blunt", 1, 2503),     // Yaksa Mace
    ("blunt", 1, 162),      // War Axe
    ("blunt", 1, 2502),     // Dwarven War Hammer
    ("blunt", 1, 89),       // Big Hammer
    ("blunt", 2, 7834),     // Art of Battle Axe
    ("blunt", 2, 171),      // Deadman's Glory
    ("blunt", 2, 175),      // Art of Battle Axe
    ("blunt", 2, 91),       // Heavy War Axe
    ("blunt", 3, 8680),     // Barakiel's Axe
    ("blunt", 3, 164),      // Elysian
    ("blunt", 3, 2504),     // Meteor Shower
    ("blunt", 4, 9448),     // Dynasty Cudgel
    ("blunt", 4, 6365),     // Basalt Battlehammer
    ("blunt", 4, 165),      // Yablonski's Hammer
    ("blunt", 5, 10220),    // Icarus Hammer
    ("blunt", 6, 15546),    // Eversor Mace
    ("blunt", 6, 15560),    // Vigwik Axe
    ("blunt", 6, 15678),    // Triumph Hammer
    ("blunt", 6, 15683),    // Triumph Staff
    ("crossbow", 0, 9227),  // Cranequin
    ("crossbow", 0, 9224),  // Arm Breaker
    ("crossbow", 0, 9221),  // Crossbow
    ("crossbow", 0, 9217),  // Hand Crossbow
    ("crossbow", 1, 9300),  // Sharpshooter
    ("crossbow", 1, 9288),  // Tathlum
    ("crossbow", 1, 9256),  // Ballista
    ("crossbow", 1, 9260),  // Ballista
    ("crossbow", 2, 9324),  // Hell Hound
    ("crossbow", 2, 9312),  // Peacemaker
    ("crossbow", 3, 9360),  // Screaming Vengeance
    ("crossbow", 3, 9348),  // Reaper
    ("crossbow", 3, 9336),  // Doomchanter
    ("crossbow", 4, 9384),  // Dynasty Crossbow
    ("crossbow", 4, 9372),  // Sarunga
    ("crossbow", 5, 10226), // Icarus Shooter
    ("crossbow", 6, 15557), // Thorne Crossbow
    ("crossbow", 6, 15571), // Dominion Crossbow
    ("crossbow", 6, 15689), // Triumph Crossbow
    ("crossbow", 6, 13469), // Vesper Shooter
    ("ancient", 0, 9226),   // General's Katzbalger
    ("ancient", 0, 9223),   // Katzbalger
    ("ancient", 0, 9220),   // Field Sword
    ("ancient", 0, 9216),   // Military Talwar
    ("ancient", 1, 9296),   // Saber Tooth
    ("ancient", 1, 9284),   // Immortal Edge
    ("ancient", 1, 9232),   // Schlager
    ("ancient", 2, 9320),   // Dismantler
    ("ancient", 2, 9308),   // Innominate Victory
    ("ancient", 3, 9356),   // Durendal
    ("ancient", 3, 9344),   // Undertaker
    ("ancient", 3, 9332),   // Divine Pain
    ("ancient", 4, 9380),   // Dynasty Ancient Sword
    ("ancient", 4, 9368),   // Gram
    ("ancient", 5, 10225),  // Icarus Wingblade
    ("ancient", 6, 15556),  // Pyseal Blade
    ("ancient", 6, 15570),  // Finale Blade
    ("ancient", 6, 15688),  // Triumph Ancientsword
    ("ancient", 6, 13470),  // Vesper Nagan
    ("rapier", 0, 9225),    // Grand Epee
    ("rapier", 0, 9222),    // Epee
    ("rapier", 0, 9218),    // Estoc
    ("rapier", 0, 9219),    // Estoc
    ("rapier", 1, 9292),    // Admiral's Estoc
    ("rapier", 1, 9264),    // Blinzlasher
    ("rapier", 1, 9268),    // Blinzlasher
    ("rapier", 1, 9272),    // Blinzlasher
    ("rapier", 2, 9316),    // Colichemarde
    ("rapier", 2, 9304),    // Military Fleuret
    ("rapier", 3, 9352),    // Éclair Bijou
    ("rapier", 3, 9340),    // Lacerator
    ("rapier", 3, 9328),    // White Lightning
    ("rapier", 4, 15687),   // Triumph Rapier
    ("rapier", 4, 9376),    // Dynasty Rapier
    ("rapier", 5, 10224),   // Icarus Stinger
    ("rapier", 6, 15555),   // Heavenstare Rapier
    ("rapier", 6, 15569),   // Gemtail Rapier
    ("rapier", 6, 13468),   // Vesper Pincer
];

/// `(weapon item id, P. Atk., M. Atk.)` of the weapons above: a companion attacks with the
/// weapon it holds, as a player does (the engine adds the stat bonus and the level modifier).
pub const WEAPON_ATTACK: &[(i32, i32, i32)] = &[
    (2499, 92, 54),    // Elven Long Sword
    (129, 79, 47),     // Sword of Revolution
    (7886, 63, 63),    // Sword of Magic Fog
    (127, 64, 39),     // Crimson Sword
    (5800, 156, 83),   // Nephilim Lord
    (135, 156, 83),    // Samurai Longsword
    (7888, 125, 111),  // Ecliptic Sword
    (75, 139, 76),     // Caliburs
    (79, 194, 99),     // Sword of Damascus
    (146, 140, 122),   // Ghoulbane
    (142, 175, 91),    // Keshanberk
    (148, 140, 122),   // Sword of Valhalla
    (8678, 251, 121),  // Sirra's Blade
    (8686, 202, 161),  // Themis' Tongue
    (151, 186, 152),   // Sword of Miracles
    (2500, 232, 114),  // Dark Legion's Edge
    (9442, 333, 151),  // Dynasty Sword
    (9444, 267, 202),  // Dynasty Phantom
    (6364, 281, 132),  // Forgotten Blade
    (10215, 363, 163), // Icarus Sawsword
    (10217, 290, 217), // Icarus Spirit
    (15544, 437, 192), // Eternal Core Sword
    (15553, 350, 256), // Archangel Sword
    (15558, 415, 183), // Periel Sword
    (15567, 332, 244), // Veniplant Sword
    (70, 112, 54),     // Claymore
    (7881, 96, 47),    // Titan Sword
    (124, 78, 39),     // Two-Handed Sword
    (5285, 49, 26),    // Heavy Sword
    (5286, 190, 83),   // Berserker Blade
    (7882, 169, 76),   // Pa'agrian Sword
    (71, 130, 61),     // Flamberge
    (7883, 236, 99),   // Guardian Sword
    (78, 213, 91),     // Great Sword
    (8679, 305, 121),  // Sword of Ipos
    (81, 282, 114),    // Dragon Slayer
    (9443, 405, 151),  // Dynasty Blade
    (21959, 405, 151), // Blood Brother
    (6372, 342, 132),  // Heaven's Divider
    (10218, 442, 163), // Icarus Heavy Arms
    (15548, 532, 192), // Lava Saw
    (15562, 505, 183), // Feather Eye Blade
    (15680, 482, 183), // Triumph Two Hand Sword
    (13458, 482, 176), // Vesper Slasher
    (159, 92, 54),     // Bonebreaker
    (189, 74, 72),     // Staff of Life
    (88, 79, 47),      // Morning Star
    (158, 79, 47),     // Tarbar
    (2503, 156, 83),   // Yaksa Mace
    (7891, 125, 111),  // Ecliptic Axe
    (162, 139, 76),    // War Axe
    (174, 111, 101),   // Nirvana Axe
    (7834, 194, 99),   // Art of Battle Axe
    (171, 194, 99),    // Deadman's Glory
    (175, 194, 99),    // Art of Battle Axe
    (91, 175, 91),     // Heavy War Axe
    (8680, 251, 121),  // Barakiel's Axe
    (8687, 202, 161),  // Cabrio's Hand
    (164, 232, 114),   // Elysian
    (7895, 186, 152),  // Flaming Dragon Skull
    (9448, 333, 151),  // Dynasty Cudgel
    (9449, 267, 202),  // Dynasty Mace
    (6365, 281, 132),  // Basalt Battlehammer
    (6579, 225, 175),  // Arcana Mace
    (10220, 363, 163), // Icarus Hammer
    (10222, 290, 217), // Icarus Hall
    (15546, 437, 192), // Eversor Mace
    (15551, 350, 256), // Sacredium
    (15560, 415, 183), // Vigwik Axe
    (15565, 332, 244), // Rising Star
    (280, 191, 54),    // Light Crossbow
    (279, 179, 51),    // Reinforced Long Bow
    (278, 132, 39),    // Gastraphetes
    (275, 114, 35),    // Long Bow
    (286, 323, 83),    // Eminence Bow
    (283, 316, 84),    // Akat Long Bow
    (282, 277, 75),    // Elemental Bow
    (285, 252, 68),    // Noble Elven Bow
    (287, 400, 99),    // Bow of Peril
    (284, 397, 100),   // Dark Elven Long Bow
    (8684, 570, 133),  // Shyeed's Bow
    (289, 528, 125),   // Soul Bow
    (288, 440, 107),   // Carnage Bow
    (9445, 654, 151),  // Dynasty Bow
    (6368, 581, 132),  // Shining Bow
    (7575, 581, 132),  // Draconic Bow
    (290, 519, 121),   // The Bow
    (10223, 689, 163), // Icarus Spitter
    (15554, 794, 192), // Recurve Thorne Bow
    (15568, 768, 183), // Skull Carnium Bow
    (15686, 724, 183), // Triumph Bow
    (13467, 724, 176), // Vesper Thrower
    (225, 80, 54),     // Mithril Dagger
    (224, 69, 47),     // Maingauche
    (223, 56, 39),     // Kukuri
    (238, 45, 52),     // Dagger of Mana
    (228, 136, 83),    // Crystal Dagger
    (231, 122, 76),    // Grace Dagger
    (233, 122, 76),    // Dark Screamer
    (227, 107, 68),    // Stiletto
    (234, 170, 99),    // Demon's Dagger
    (229, 153, 91),    // Kris
    (243, 122, 122),   // Hell Knife
    (8682, 220, 121),  // Naga Storm
    (236, 203, 114),   // Soul Separator
    (235, 186, 107),   // Bloody Orchid
    (9446, 291, 151),  // Dynasty Knife
    (6367, 246, 132),  // Angel Slayer
    (237, 220, 121),   // Dragon's Tooth
    (10216, 318, 163), // Icarus Disperser
    (15545, 382, 192), // Mamba Edge
    (15559, 363, 183), // Skull Edge
    (15677, 346, 183), // Triumph Dagger
    (13460, 346, 176), // Vesper Shaper
    (187, 90, 79),     // Atuba Hammer
    (188, 90, 79),     // Ghost Staff
    (190, 90, 79),     // Atuba Mace
    (204, 152, 122),   // Deadman's Staff
    (205, 152, 122),   // Ghoul's Staff
    (206, 152, 122),   // Demon's Staff
    (209, 189, 132),   // Divine Staff
    (211, 189, 132),   // Staff of Nobility
    (210, 189, 145),   // Staff of Evil Spirits
    (8688, 245, 177),  // Daimon Crystal
    (10252, 325, 222), // Dynasty Staff
    (6366, 274, 193),  // Imperial Staff
    (15552, 426, 281), // Cyclic Cane
    (2529, 107, 51),   // Bastard Sword*Crimson Sword
    (2530, 107, 51),   // Bastard Sword*Elven Sword
    (2536, 107, 51),   // Spinebone Sword*Crimson Sword
    (2537, 107, 51),   // Spinebone Sword*Elven Sword
    (2572, 190, 83),   // Shamshir*Shamshir
    (2573, 190, 83),   // Shamshir*Katana
    (2574, 190, 83),   // Shamshir*Spirit Sword
    (2575, 190, 83),   // Shamshir*Raid Sword
    (2626, 236, 99),   // Samurai Long Sword*Samurai Long Sword
    (2611, 228, 97),   // Caliburs*Samurai Long Sword
    (2616, 228, 97),   // Sword of Limit*Samurai Long Sword
    (2620, 228, 97),   // Sword of Delusion*Samurai Long Sword
    (5706, 282, 114),  // Damascus*Damascus
    (5705, 275, 112),  // Keshanberk*Damascus
    (5704, 259, 107),  // Keshanberk*Keshanberk
    (5233, 259, 107),  // Keshanberk*Keshanberk
    (10004, 405, 151), // Dynasty Dual Sword
    (21955, 405, 151), // Blades of Delusion
    (6580, 342, 132),  // Tallum Blade*Dark Legion's Edge
    (10415, 442, 163), // Icarus Dual Sword
    (16158, 532, 192), // Eternal Core Dual Sword
    (16154, 505, 183), // Periel Dual Sword
    (52, 482, 176),    // Vesper Dual Sword
    (297, 92, 54),     // Glaive
    (93, 79, 47),      // Winged Spear
    (294, 79, 47),     // War Pick
    (293, 64, 39),     // War Hammer
    (299, 156, 83),    // Orcish Poleaxe
    (301, 144, 78),    // Scorpion
    (303, 144, 78),    // Widow Maker
    (95, 139, 76),     // Poleaxe
    (97, 194, 99),     // Lance
    (300, 175, 91),    // Great Axe
    (8683, 251, 121),  // Tiphon's Spear
    (304, 219, 109),   // Orcish Halberd
    (5629, 219, 109),  // Orcish Halberd
    (5630, 219, 109),  // Orcish Halberd
    (9447, 333, 151),  // Dynasty Halberd
    (6370, 281, 132),  // Saint Spear
    (306, 251, 121),   // Dragon Claw Axe
    (10219, 363, 163), // Icarus Trident
    (15550, 437, 192), // Demitelum
    (15564, 415, 183), // Doubletop Spear
    (15682, 396, 183), // Triumph Spear
    (13462, 396, 176), // Vesper Stormer
    (262, 112, 54),    // Scallop Jamadhr
    (261, 96, 47),     // Bich'Hwa
    (260, 78, 39),     // Triple-Edged Jamadhr
    (259, 62, 32),     // Single-Edged Jamadhr
    (266, 190, 83),    // Great Pata
    (265, 169, 76),    // Fisted Blade
    (4233, 148, 68),   // Knuckle Duster
    (263, 130, 61),    // Chakram
    (268, 236, 99),    // Bellion Cestus
    (264, 204, 89),    // Pata
    (267, 213, 91),    // Arthro Nail
    (8685, 305, 121),  // Sobekk's Hurricane
    (270, 282, 114),   // Dragon Grinder
    (269, 259, 107),   // Blood Tornado
    (9450, 405, 151),  // Dynasty Bagh-Nakh
    (6371, 342, 132),  // Demon Splinter
    (10221, 442, 163), // Icarus Hand
    (15549, 532, 192), // Jade Claw
    (15563, 505, 183), // Octo Claw
    (15681, 482, 183), // Triumph Jamadhr
    (13461, 482, 176), // Vesper Fighter
    (169, 79, 47),     // Skull Breaker
    (2502, 122, 68),   // Dwarven War Hammer
    (89, 107, 61),     // Big Hammer
    (2504, 213, 107),  // Meteor Shower
    (165, 251, 121),   // Yablonski's Hammer
    (15678, 396, 183), // Triumph Hammer
    (15683, 317, 244), // Triumph Staff
    (9227, 117, 54),   // Cranequin
    (9224, 100, 47),   // Arm Breaker
    (9221, 81, 39),    // Crossbow
    (9217, 64, 32),    // Hand Crossbow
    (9300, 198, 83),   // Sharpshooter
    (9288, 176, 76),   // Tathlum
    (9256, 155, 68),   // Ballista
    (9260, 155, 68),   // Ballista
    (9324, 245, 99),   // Hell Hound
    (9312, 221, 91),   // Peacemaker
    (9360, 318, 121),  // Screaming Vengeance
    (9348, 294, 114),  // Reaper
    (9336, 270, 107),  // Doomchanter
    (9384, 401, 151),  // Dynasty Crossbow
    (9372, 356, 132),  // Sarunga
    (10226, 422, 163), // Icarus Shooter
    (15557, 487, 192), // Thorne Crossbow
    (15571, 471, 183), // Dominion Crossbow
    (15689, 444, 183), // Triumph Crossbow
    (13469, 444, 176), // Vesper Shooter
    (9226, 100, 54),   // General's Katzbalger
    (9223, 86, 47),    // Katzbalger
    (9220, 69, 39),    // Field Sword
    (9216, 55, 32),    // Military Talwar
    (9296, 169, 83),   // Saber Tooth
    (9284, 151, 76),   // Immortal Edge
    (9232, 116, 61),   // Schlager
    (9320, 210, 99),   // Dismantler
    (9308, 190, 91),   // Innominate Victory
    (9356, 272, 121),  // Durendal
    (9344, 251, 114),  // Undertaker
    (9332, 231, 107),  // Divine Pain
    (9380, 361, 151),  // Dynasty Ancient Sword
    (9368, 304, 132),  // Gram
    (10225, 393, 163), // Icarus Wingblade
    (15556, 473, 192), // Pyseal Blade
    (15570, 449, 183), // Finale Blade
    (15688, 429, 183), // Triumph Ancientsword
    (13470, 429, 176), // Vesper Nagan
    (9225, 83, 54),    // Grand Epee
    (9222, 72, 47),    // Epee
    (9218, 58, 39),    // Estoc
    (9219, 58, 39),    // Estoc
    (9292, 141, 83),   // Admiral's Estoc
    (9264, 126, 76),   // Blinzlasher
    (9268, 126, 76),   // Blinzlasher
    (9272, 126, 76),   // Blinzlasher
    (9316, 176, 99),   // Colichemarde
    (9304, 159, 91),   // Military Fleuret
    (9352, 228, 121),  // Éclair Bijou
    (9340, 210, 114),  // Lacerator
    (9328, 193, 107),  // White Lightning
    (15687, 344, 183), // Triumph Rapier
    (9376, 302, 151),  // Dynasty Rapier
    (10224, 329, 163), // Icarus Stinger
    (15555, 396, 192), // Heavenstare Rapier
    (15569, 376, 183), // Gemtail Rapier
    (13468, 359, 176), // Vesper Pincer
];

/// `(grade index, armor kind, [head, chest, legs, gloves, feet, shield])`; 0 is nothing.
pub const ARMOR: &[(usize, &str, [i32; 6])] = &[
    (0, "HEAVY", [47, 58, 59, 0, 0, 628]),              // Mithril Breastplate
    (0, "LIGHT", [0, 394, 416, 0, 2422, 0]),            // Reinforced Leather Shirt
    (0, "MAGIC", [0, 436, 469, 2447, 0, 0]),            // Tunic of Knowledge
    (0, "LIGHT", [0, 395, 417, 0, 2424, 0]),            // Manticore Skin Shirt
    (0, "HEAVY", [2411, 352, 2378, 0, 0, 2493]),        // Brigandine Tunic
    (0, "MAGIC", [0, 437, 470, 2450, 0, 0]),            // Mithril Tunic
    (1, "LIGHT", [0, 397, 2387, 0, 62, 0]),             // Mithril Shirt
    (1, "HEAVY", [2413, 354, 381, 0, 0, 2495]),         // Chain Mail Shirt
    (1, "MAGIC", [0, 439, 471, 2454, 0, 0]),            // Karmian Tunic
    (1, "LIGHT", [0, 398, 418, 0, 2431, 0]),            // Plated Leather
    (1, "HEAVY", [517, 60, 0, 0, 0, 107]),              // Composite Armor
    (1, "MAGIC", [0, 441, 472, 2459, 0, 0]),            // Demon's Tunic
    (1, "LIGHT", [0, 400, 420, 0, 2436, 0]),            // Theca Leather Armor
    (1, "LIGHT", [0, 401, 0, 0, 2437, 0]),              // Drake Leather Armor
    (1, "HEAVY", [2414, 356, 0, 0, 0, 2497]),           // Full Plate Armor
    (1, "MAGIC", [0, 442, 473, 2463, 0, 0]),            // Divine Tunic
    (2, "HEAVY", [503, 357, 383, 5710, 5726, 0]),       // Zubei's Breastplate
    (2, "HEAVY", [2415, 2376, 2379, 5714, 5730, 673]),  // Avadon Breastplate
    (2, "LIGHT", [503, 2384, 2388, 5711, 5727, 0]),     // Zubei's Leather Shirt
    (2, "LIGHT", [2415, 2390, 0, 5715, 5731, 0]),       // Avadon Leather Armor
    (2, "MAGIC", [503, 2397, 2402, 5712, 5728, 0]),     // Tunic of Zubei
    (2, "MAGIC", [2415, 2406, 0, 5716, 5732, 0]),       // Avadon Robe
    (2, "HEAVY", [2416, 358, 2380, 5718, 5734, 0]),     // Blue Wolf Breastplate
    (2, "HEAVY", [2417, 2381, 0, 5722, 5738, 110]),     // Doom Plate Armor
    (2, "LIGHT", [2416, 2391, 0, 5719, 5735, 0]),       // Blue Wolf Leather Armor
    (2, "LIGHT", [2417, 2392, 0, 5723, 5739, 0]),       // Leather Armor of Doom
    (2, "MAGIC", [2416, 2398, 2403, 5720, 5736, 0]),    // Blue Wolf Tunic
    (2, "MAGIC", [2417, 2399, 2404, 5724, 5740, 0]),    // Tunic of Doom
    (3, "HEAVY", [512, 365, 388, 5765, 5777, 641]),     // Dark Crystal Breastplate
    (3, "HEAVY", [547, 2382, 0, 5768, 5780, 0]),        // Tallum Plate Armor
    (3, "LIGHT", [512, 2385, 2389, 5766, 5778, 0]),     // Dark Crystal Leather Armor
    (3, "LIGHT", [547, 2393, 0, 5769, 5781, 0]),        // Tallum Leather Armor
    (3, "MAGIC", [547, 2400, 2405, 5770, 5782, 0]),     // Tallum Tunic
    (3, "MAGIC", [512, 2407, 0, 5767, 5779, 0]),        // Dark Crystal Robe
    (3, "HEAVY", [2418, 374, 0, 5771, 5783, 2498]),     // Armor of Nightmare
    (3, "HEAVY", [2419, 2383, 0, 5774, 5786, 0]),       // Majestic Plate Armor
    (3, "LIGHT", [2418, 2394, 0, 5772, 5784, 0]),       // Leather Armor of Nightmare
    (3, "LIGHT", [2419, 2395, 0, 5775, 5787, 0]),       // Majestic Leather Armor
    (3, "MAGIC", [2418, 2408, 0, 5773, 5785, 0]),       // Robe of Nightmare
    (3, "MAGIC", [2419, 2409, 0, 5776, 5788, 0]),       // Majestic Robe
    (4, "HEAVY", [6378, 6373, 6374, 6375, 6376, 6377]), // Imperial Crusader Breastplate
    (4, "LIGHT", [6382, 6379, 0, 6380, 6381, 0]),       // Draconic Leather Armor
    (4, "MAGIC", [6386, 6383, 0, 6384, 6385, 0]),       // Major Arcana Robe
    (4, "HEAVY", [9422, 9416, 9421, 9423, 9424, 9441]), // Dynasty Breast Plate
    (4, "LIGHT", [9429, 9425, 9428, 9430, 9431, 0]),    // Dynasty Leather Armor
    (4, "MAGIC", [9438, 9432, 9437, 9439, 9440, 0]),    // Dynasty Tunic
    (5, "HEAVY", [15606, 15609, 15612, 15615, 15618, 15621]), // Moirai Breastplate
    (5, "LIGHT", [15607, 15610, 15613, 15616, 15619, 0]), // Moirai Leather Breastplate
    (5, "MAGIC", [15608, 15611, 15614, 15617, 15620, 0]), // Moirai Tunic
    (6, "HEAVY", [13137, 13432, 13438, 13439, 13440, 13471]), // Vesper Breastplate
    (6, "LIGHT", [13138, 13433, 13441, 13442, 13443, 0]), // Vesper Leather Breastplate
    (6, "MAGIC", [13139, 13434, 13444, 13445, 13446, 0]), // Vesper Tunic
    (6, "HEAVY", [13140, 13435, 13448, 13449, 13450, 13471]), // Vesper Noble Breastplate
    (6, "LIGHT", [13141, 13436, 13451, 13452, 13453, 0]), // Vesper Noble Leather Breastplate
    (6, "MAGIC", [13142, 13437, 13454, 13455, 13456, 0]), // Vesper Noble Tunic
    (6, "HEAVY", [15589, 15592, 15595, 15598, 15601, 15604]), // Vorpal Breastplate
    (6, "LIGHT", [15590, 15593, 15596, 15599, 15602, 0]), // Vorpal Leather Breastplate
    (6, "MAGIC", [15591, 15594, 15597, 15600, 15603, 0]), // Vorpal Tunic
    (6, "HEAVY", [15572, 15575, 15578, 15581, 15584, 15587]), // Elegia Breastplate
    (6, "LIGHT", [15573, 15576, 15579, 15582, 15585, 0]), // Elegia Leather Breastplate
    (6, "MAGIC", [15574, 15577, 15580, 15583, 15586, 0]), // Elegia Tunic
];
