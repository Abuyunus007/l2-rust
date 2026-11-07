//! Generated from the item and armor set data of the datapack (`data/stats/items`, `armorsets`):
//! the weapons and armor a companion wears, by grade. Item ids only; the test checks each one
//! against the datapack.

/// `(role key, grade index, weapon item id)`. The grades are D, C, B, A, S, S80, S84.
pub const WEAPONS: &[(&str, usize, i32)] = &[
    ("tank", 0, 2499),     // Elven Long Sword
    ("tank", 0, 129),      // Sword of Revolution
    ("tank", 0, 7886),     // Sword of Magic Fog
    ("tank", 0, 127),      // Crimson Sword
    ("tank", 1, 5800),     // Nephilim Lord
    ("tank", 1, 135),      // Samurai Longsword
    ("tank", 1, 7888),     // Ecliptic Sword
    ("tank", 1, 75),       // Caliburs
    ("tank", 2, 79),       // Sword of Damascus
    ("tank", 2, 146),      // Ghoulbane
    ("tank", 2, 142),      // Keshanberk
    ("tank", 2, 148),      // Sword of Valhalla
    ("tank", 3, 8678),     // Sirra's Blade
    ("tank", 3, 8686),     // Themis' Tongue
    ("tank", 3, 151),      // Sword of Miracles
    ("tank", 3, 2500),     // Dark Legion's Edge
    ("tank", 4, 9442),     // Dynasty Sword
    ("tank", 4, 9444),     // Dynasty Phantom
    ("tank", 4, 6364),     // Forgotten Blade
    ("tank", 5, 10215),    // Icarus Sawsword
    ("tank", 5, 10217),    // Icarus Spirit
    ("tank", 6, 15544),    // Eternal Core Sword
    ("tank", 6, 15553),    // Archangel Sword
    ("tank", 6, 15558),    // Periel Sword
    ("tank", 6, 15567),    // Veniplant Sword
    ("warrior", 0, 70),    // Claymore
    ("warrior", 0, 7881),  // Titan Sword
    ("warrior", 0, 124),   // Two-Handed Sword
    ("warrior", 0, 5285),  // Heavy Sword
    ("warrior", 1, 5286),  // Berserker Blade
    ("warrior", 1, 7882),  // Pa'agrian Sword
    ("warrior", 1, 71),    // Flamberge
    ("warrior", 2, 7883),  // Guardian Sword
    ("warrior", 2, 78),    // Great Sword
    ("warrior", 3, 8679),  // Sword of Ipos
    ("warrior", 3, 81),    // Dragon Slayer
    ("warrior", 4, 9443),  // Dynasty Blade
    ("warrior", 4, 21959), // Blood Brother
    ("warrior", 4, 6372),  // Heaven's Divider
    ("warrior", 5, 10218), // Icarus Heavy Arms
    ("warrior", 6, 15548), // Lava Saw
    ("warrior", 6, 15562), // Feather Eye Blade
    ("warrior", 6, 15680), // Triumph Two Hand Sword
    ("warrior", 6, 13458), // Vesper Slasher
    ("healer", 0, 159),    // Bonebreaker
    ("healer", 0, 189),    // Staff of Life
    ("healer", 0, 88),     // Morning Star
    ("healer", 0, 158),    // Tarbar
    ("healer", 1, 2503),   // Yaksa Mace
    ("healer", 1, 7891),   // Ecliptic Axe
    ("healer", 1, 162),    // War Axe
    ("healer", 1, 174),    // Nirvana Axe
    ("healer", 2, 7834),   // Art of Battle Axe
    ("healer", 2, 171),    // Deadman's Glory
    ("healer", 2, 175),    // Art of Battle Axe
    ("healer", 2, 91),     // Heavy War Axe
    ("healer", 3, 8680),   // Barakiel's Axe
    ("healer", 3, 8687),   // Cabrio's Hand
    ("healer", 3, 164),    // Elysian
    ("healer", 3, 7895),   // Flaming Dragon Skull
    ("healer", 4, 9448),   // Dynasty Cudgel
    ("healer", 4, 9449),   // Dynasty Mace
    ("healer", 4, 6365),   // Basalt Battlehammer
    ("healer", 4, 6579),   // Arcana Mace
    ("healer", 5, 10220),  // Icarus Hammer
    ("healer", 5, 10222),  // Icarus Hall
    ("healer", 6, 15546),  // Eversor Mace
    ("healer", 6, 15551),  // Sacredium
    ("healer", 6, 15560),  // Vigwik Axe
    ("healer", 6, 15565),  // Rising Star
    ("archer", 0, 280),    // Light Crossbow
    ("archer", 0, 279),    // Reinforced Long Bow
    ("archer", 0, 278),    // Gastraphetes
    ("archer", 0, 275),    // Long Bow
    ("archer", 1, 286),    // Eminence Bow
    ("archer", 1, 283),    // Akat Long Bow
    ("archer", 1, 282),    // Elemental Bow
    ("archer", 1, 285),    // Noble Elven Bow
    ("archer", 2, 287),    // Bow of Peril
    ("archer", 2, 284),    // Dark Elven Long Bow
    ("archer", 3, 8684),   // Shyeed's Bow
    ("archer", 3, 289),    // Soul Bow
    ("archer", 3, 288),    // Carnage Bow
    ("archer", 4, 9445),   // Dynasty Bow
    ("archer", 4, 6368),   // Shining Bow
    ("archer", 4, 7575),   // Draconic Bow
    ("archer", 4, 290),    // The Bow
    ("archer", 5, 10223),  // Icarus Spitter
    ("archer", 6, 15554),  // Recurve Thorne Bow
    ("archer", 6, 15568),  // Skull Carnium Bow
    ("archer", 6, 15686),  // Triumph Bow
    ("archer", 6, 13467),  // Vesper Thrower
    ("dagger", 0, 225),    // Mithril Dagger
    ("dagger", 0, 224),    // Maingauche
    ("dagger", 0, 223),    // Kukuri
    ("dagger", 0, 238),    // Dagger of Mana
    ("dagger", 1, 228),    // Crystal Dagger
    ("dagger", 1, 231),    // Grace Dagger
    ("dagger", 1, 233),    // Dark Screamer
    ("dagger", 1, 227),    // Stiletto
    ("dagger", 2, 234),    // Demon's Dagger
    ("dagger", 2, 229),    // Kris
    ("dagger", 2, 243),    // Hell Knife
    ("dagger", 3, 8682),   // Naga Storm
    ("dagger", 3, 236),    // Soul Separator
    ("dagger", 3, 235),    // Bloody Orchid
    ("dagger", 4, 9446),   // Dynasty Knife
    ("dagger", 4, 6367),   // Angel Slayer
    ("dagger", 4, 237),    // Dragon's Tooth
    ("dagger", 5, 10216),  // Icarus Disperser
    ("dagger", 6, 15545),  // Mamba Edge
    ("dagger", 6, 15559),  // Skull Edge
    ("dagger", 6, 15677),  // Triumph Dagger
    ("dagger", 6, 13460),  // Vesper Shaper
    ("mage", 0, 187),      // Atuba Hammer
    ("mage", 0, 188),      // Ghost Staff
    ("mage", 0, 189),      // Staff of Life
    ("mage", 0, 190),      // Atuba Mace
    ("mage", 1, 204),      // Deadman's Staff
    ("mage", 1, 205),      // Ghoul's Staff
    ("mage", 1, 206),      // Demon's Staff
    ("mage", 1, 7888),     // Ecliptic Sword
    ("mage", 2, 209),      // Divine Staff
    ("mage", 2, 211),      // Staff of Nobility
    ("mage", 2, 210),      // Staff of Evil Spirits
    ("mage", 2, 146),      // Ghoulbane
    ("mage", 3, 8686),     // Themis' Tongue
    ("mage", 3, 8687),     // Cabrio's Hand
    ("mage", 3, 8688),     // Daimon Crystal
    ("mage", 3, 151),      // Sword of Miracles
    ("mage", 4, 9444),     // Dynasty Phantom
    ("mage", 4, 9449),     // Dynasty Mace
    ("mage", 4, 10252),    // Dynasty Staff
    ("mage", 4, 6366),     // Imperial Staff
    ("mage", 5, 10217),    // Icarus Spirit
    ("mage", 5, 10222),    // Icarus Hall
    ("mage", 6, 15551),    // Sacredium
    ("mage", 6, 15552),    // Cyclic Cane
    ("mage", 6, 15553),    // Archangel Sword
    ("mage", 6, 15565),    // Rising Star
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
