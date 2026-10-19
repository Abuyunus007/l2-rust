package l2rust.vectors;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import org.l2jmobius.commons.database.DatabaseFactory;
import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.entity.actor.Creature;
import org.l2jmobius.gameserver.entity.actor.Npc;
import org.l2jmobius.gameserver.entity.actor.Player;
import org.l2jmobius.gameserver.entity.itemcontainer.Inventory;
import org.l2jmobius.gameserver.mechanics.skill.Skill;
import org.l2jmobius.gameserver.mechanics.stats.BaseStat;
import org.l2jmobius.gameserver.mechanics.stats.Stat;

/**
 * Stat engine vectors (M4-JV-01): every number {@code UserInfo} / {@code NpcInfo} reads, computed by the real {@code CreatureStat}, {@code Calculator}, {@code Func*}, skill passives, item stat functions
 * and armor sets of L2JMobius, for fixed players and NPCs. Input (class, level, items with enchant, skills) and output are in the same case object.
 * <p>
 * Files: {@code stats_players_naked.json}, {@code stats_players_gear.json}, {@code stats_npcs.json}, {@code stats_base_bonus.json}. Needs the game server root as the working directory (see {@link Bootstrap}).
 */
public final class StatVectors
{
	private StatVectors()
	{
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Case specification
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	/** What to build: a player (class, sex, level, gear, passive skills) or an NPC. */
	public static final class Spec
	{
		public final String id;
		public final int classId;
		public final boolean female;
		public final int level;
		/** {@code [itemId, enchant]} in equip order. */
		public final int[][] items;
		/** {@code true}: give the auto-get skills of the class tree for the level (what the server's {@code rewardSkills} gives with the shipped config). */
		public final boolean skills;
		public final int npcId;

		private Spec(String id, int classId, boolean female, int level, int[][] items, boolean skills, int npcId)
		{
			this.id = id;
			this.classId = classId;
			this.female = female;
			this.level = level;
			this.items = items;
			this.skills = skills;
			this.npcId = npcId;
		}

		public static Spec player(String id, int classId, boolean female, int level, int[][] items, boolean skills)
		{
			return new Spec(id, classId, female, level, items, skills, 0);
		}

		public static Spec npc(String id, int npcId)
		{
			return new Spec(id, 0, false, 0, new int[0][], false, npcId);
		}

		public boolean isNpc()
		{
			return npcId != 0;
		}
	}

	/** A built creature plus the facts about how it was built. */
	public static final class Built
	{
		public final Spec spec;
		public Creature creature;
		/** Auto-get skills in the order their functions survive in the calculators, {@code [id, level]}. */
		public final List<int[]> skills = new ArrayList<>();
		/** Skills the player knows after gear and expertise (armor set skills, item skills, grade penalty, ...) that are not in {@link #skills} with the same level. */
		public final List<int[]> derivedSkills = new ArrayList<>();

		Built(Spec spec)
		{
			this.spec = spec;
		}
	}

	/** Item sets used by the cases (all ids exist in the shipped datapack; the generator aborts otherwise). */
	public static final int[][] GEAR_HEAVY_A = gear(148, 365, 388, 512, 5765, 5777, 641, 858, 858, 920, 889, 889);
	public static final int[][] GEAR_LIGHT_BOW_A = gear(7575, 2393, 0, 547, 5769, 5781, 0, 858, 858, 920, 889, 889);
	public static final int[][] GEAR_ROBE_A = gear(6579, 2409, 0, 2419, 5776, 5788, 0, 862, 862, 924, 893, 893);
	public static final int[][] GEAR_LIGHT_DAGGER_A = gear(226, 2393, 0, 547, 5769, 5781, 0, 0, 0, 0, 0, 0);

	/** weapon, chest, legs, head, gloves, boots, shield, lear, rear, neck, lring, rring; 0 = empty slot. Enchant is added by {@link #enchanted}. */
	private static int[][] gear(int... ids)
	{
		final List<int[]> list = new ArrayList<>();
		for (int id : ids)
		{
			if (id != 0)
			{
				list.add(new int[]
				{
					id,
					0
				});
			}
		}

		return list.toArray(new int[0][]);
	}

	/** The same gear with every piece at the given enchant level. */
	public static int[][] enchanted(int[][] gear, int enchant)
	{
		final int[][] copy = new int[gear.length][];
		for (int i = 0; i < gear.length; i++)
		{
			copy[i] = new int[]
			{
				gear[i][0],
				enchant
			};
		}

		return copy;
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Building
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	/**
	 * Builds the creature the way the server would have it after login: player constructor, level, fists weapon; gear through {@code Inventory.equipItem}; skills like {@code rewardSkills};
	 * then {@code refreshExpertisePenalty} and {@code refreshOverloaded}. NPCs through the real class named by the template type.
	 * @return the built creature, or {@code null} when the class or NPC has no template
	 */
	public static Built build(Spec spec) throws Exception
	{
		final Built built = new Built(spec);
		if (spec.isNpc())
		{
			final Npc npc = Bootstrap.newNpc(spec.npcId);
			if (npc == null)
			{
				return null;
			}

			built.creature = npc;
			return built;
		}

		final Player player = Bootstrap.newPlayer(spec.classId, spec.female, spec.level);
		if (player == null)
		{
			return null;
		}

		built.creature = player;

		// Player.restore order: inventory (equip), then skills.
		for (int[] item : spec.items)
		{
			Bootstrap.equip(player, item[0], item[1]);
		}

		final Method refreshWeight = Inventory.class.getSuperclass().getDeclaredMethod("refreshWeight");
		refreshWeight.setAccessible(true);
		refreshWeight.invoke(player.getInventory());

		List<int[]> sequence = new ArrayList<>();
		if (spec.skills)
		{
			// Player.restore finds the skills of earlier level-ups in the database, so a second pass only adds higher levels. Without a database one pass adds every level of a
			// multi-level skill in map order and the last one wins; repeat until nothing is added so every skill ends at its highest level.
			for (int pass = 0; pass < 20; pass++)
			{
				final List<int[]> added = Bootstrap.giveSkills(player);
				if (added.isEmpty())
				{
					break;
				}

				sequence.addAll(added);
			}
		}

		// Only the last add of each skill id leaves functions behind.
		for (int i = 0; i < sequence.size(); i++)
		{
			boolean last = true;
			for (int j = i + 1; j < sequence.size(); j++)
			{
				if (sequence.get(j)[0] == sequence.get(i)[0])
				{
					last = false;
					break;
				}
			}

			if (last)
			{
				built.skills.add(sequence.get(i));
			}
		}

		player.refreshExpertisePenalty();
		player.refreshOverloaded();

		final List<Skill> known = new ArrayList<>(player.getSkills().values());
		known.sort((a, b) -> Integer.compare(a.getId(), b.getId()));
		for (Skill skill : known)
		{
			boolean listed = false;
			for (int[] s : built.skills)
			{
				if ((s[0] == skill.getId()) && (s[1] == skill.getLevel()))
				{
					listed = true;
					break;
				}
			}

			if (!listed)
			{
				built.derivedSkills.add(new int[]
				{
					skill.getId(),
					skill.getLevel()
				});
			}
		}

		return built;
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Dumping
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	/** Stats probed with the init value the formulas use: {@code "STAT_NAME:init"}. They expose the add/mul functions behind formula inputs that have no getter. */
	private static final String[] PROBES =
	{
		"CRITICAL_DAMAGE:1",
		"CRITICAL_DAMAGE_POS:1",
		"CRITICAL_DAMAGE_ADD:0",
		"DEFENCE_CRITICAL_DAMAGE:1",
		"DEFENCE_CRITICAL_DAMAGE_ADD:0",
		"DEFENCE_CRITICAL_RATE:100",
		"DEFENCE_CRITICAL_RATE_ADD:0",
		"CRITICAL_RATE_POS:100",
		"SHIELD_RATE:0",
		"SHIELD_DEFENCE_ANGLE:0",
		"RANDOM_DAMAGE:0",
		"ATTACK_CANCEL:50",
		"REGENERATE_HP_RATE:100",
		"REGENERATE_MP_RATE:100",
		"REGENERATE_CP_RATE:100",
		"PVE_PHYSICAL_DMG:1",
		"PVE_BOW_DMG:1",
		"PVP_PHYSICAL_DMG:1",
		"PVP_PHYSICAL_DEF:1",
		"PVP_PHYS_SKILL_DMG:1",
		"POWER_ATTACK_RANGE:0",
		"ATK_REUSE:1",
		"MAGIC_CRIT_DMG:1",
		"P_SKILL_EVASION:0",
		"MAGIC_SUCCESS_RES:1",
		"WEIGHT_LIMIT:1",
		"WEIGHT_PENALTY:1",
		"BLOW_RATE:0",
		"SKILL_MASTERY:0",
		"MAX_RECOVERABLE_HP:0",
		"MAX_RECOVERABLE_MP:0",
	};

	/** All numbers of a creature, as the packets carry them ({@code user_info}) and as raw doubles ({@code raw}). */
	public static Map<String, Object> dump(Creature c)
	{
		final Map<String, Object> o = Json.obj();
		final boolean player = c.isPlayer();

		// UserInfo / NpcInfo: what the client gets.
		final double mult = c.getMovementSpeedMultiplier();
		final Map<String, Object> ui = Json.obj();
		ui.put("str", c.getSTR());
		ui.put("dex", c.getDEX());
		ui.put("con", c.getCON());
		ui.put("int", c.getINT());
		ui.put("wit", c.getWIT());
		ui.put("men", c.getMEN());
		ui.put("max_hp", c.getMaxHp());
		ui.put("max_mp", c.getMaxMp());
		if (player)
		{
			ui.put("max_cp", c.getMaxCp());
			ui.put("max_load", c.getMaxLoad());
		}

		ui.put("p_atk", (int) c.getPAtk(null));
		ui.put("p_atk_spd", (int) c.getPAtkSpd());
		ui.put("p_def", (int) c.getPDef(null));
		ui.put("evasion", c.getEvasionRate(null));
		ui.put("accuracy", c.getAccuracy());
		ui.put("critical", c.getCriticalHit(null, null));
		ui.put("m_atk", (int) c.getMAtk(null, null));
		ui.put("m_atk_spd", c.getMAtkSpd());
		ui.put("m_def", (int) c.getMDef(null, null));
		ui.put("run_speed", (int) Math.round(c.getRunSpeed() / mult));
		ui.put("walk_speed", (int) Math.round(c.getWalkSpeed() / mult));
		ui.put("swim_run_speed", (int) Math.round(c.getSwimRunSpeed() / mult));
		ui.put("swim_walk_speed", (int) Math.round(c.getSwimWalkSpeed() / mult));
		ui.put("attack_range", c.getPhysicalAttackRange());
		o.put("user_info", Json.compact(ui));

		// Full doubles.
		final Map<String, Object> raw = Json.obj();
		raw.put("p_atk", c.getPAtk(null));
		raw.put("p_atk_spd", c.getPAtkSpd());
		raw.put("p_def", c.getPDef(null));
		raw.put("m_atk", c.getMAtk(null, null));
		raw.put("m_def", c.getMDef(null, null));
		raw.put("run_speed", c.getRunSpeed());
		raw.put("walk_speed", c.getWalkSpeed());
		raw.put("swim_run_speed", c.getSwimRunSpeed());
		raw.put("swim_walk_speed", c.getSwimWalkSpeed());
		raw.put("movement_speed_multiplier", mult);
		raw.put("attack_speed_multiplier", c.getAttackSpeedMultiplier());
		raw.put("m_critical", c.getMCriticalHit(null, null));
		raw.put("shield_def", c.getShldDef());
		raw.put("level_mod", c.getLevelMod());
		raw.put("level", c.getLevel());
		raw.put("collision_radius", player ? (double) c.asPlayer().getCollisionRadius() : c.asNpc().getCollisionRadius());
		raw.put("collision_height", player ? (double) c.asPlayer().getCollisionHeight() : c.asNpc().getCollisionHeight());
		if (player)
		{
			raw.put("inventory_limit", c.asPlayer().getInventoryLimit());
			raw.put("current_load", c.getCurrentLoad());
		}

		final Map<String, Object> bonus = Json.obj();
		bonus.put("str", BaseStat.STR.calcBonus(c));
		bonus.put("dex", BaseStat.DEX.calcBonus(c));
		bonus.put("con", BaseStat.CON.calcBonus(c));
		bonus.put("int", BaseStat.INT.calcBonus(c));
		bonus.put("wit", BaseStat.WIT.calcBonus(c));
		bonus.put("men", BaseStat.MEN.calcBonus(c));
		raw.put("base_stat_bonus", Json.compact(bonus));

		final Map<String, Object> probes = Json.obj();
		for (String probe : PROBES)
		{
			final String[] parts = probe.split(":");
			final Stat stat = Stat.valueOf(parts[0]);
			probes.put(stat.name().toLowerCase() + "@" + parts[1], c.calcStat(stat, Double.parseDouble(parts[1])));
		}

		raw.put("probes", Json.compact(probes));
		o.put("raw", raw);
		return o;
	}

	/** The input half of a case: what Rust has to rebuild. */
	public static Map<String, Object> input(Built b)
	{
		final Spec spec = b.spec;
		final Map<String, Object> in = Json.obj();
		if (spec.isNpc())
		{
			in.put("kind", "npc");
			in.put("npc_id", spec.npcId);
			final Npc npc = b.creature.asNpc();
			in.put("npc_name", npc.getTemplate().getName());
			in.put("npc_type", npc.getTemplate().getType());
			in.put("level", (int) npc.getTemplate().getLevel());
			return in;
		}

		in.put("kind", "player");
		in.put("class_id", spec.classId);
		in.put("female", spec.female);
		in.put("level", spec.level);
		final List<Object> items = new ArrayList<>();
		for (int[] item : spec.items)
		{
			final Map<String, Object> i = Json.obj();
			i.put("item_id", item[0]);
			i.put("enchant", item[1]);
			items.add(Json.compact(i));
		}

		in.put("items", items);
		in.put("skills", skillList(b.skills));
		in.put("derived_skills", skillList(b.derivedSkills));
		return in;
	}

	public static List<Object> skillList(List<int[]> skills)
	{
		final List<Object> list = new ArrayList<>();
		for (int[] s : skills)
		{
			final Map<String, Object> m = Json.obj();
			m.put("id", s[0]);
			m.put("level", s[1]);
			list.add(Json.compact(m));
		}

		return list;
	}

	/** One case: id, input and expected output. */
	public static Map<String, Object> caseOf(Built b)
	{
		final Map<String, Object> c = Json.obj();
		c.put("id", b.spec.id);
		c.put("input", input(b));
		c.put("expected", dump(b.creature));
		return c;
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Case lists
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	/** The 11 starting classes of High Five. */
	public static final int[] STARTING_CLASSES =
	{
		0,
		10,
		18,
		25,
		31,
		38,
		44,
		49,
		53,
		123,
		124
	};

	/** First profession classes (level 20 cases). */
	private static final int[] CLASSES_L20 =
	{
		1,
		4,
		7,
		11,
		15,
		19,
		22,
		26,
		29,
		32,
		35,
		39,
		42,
		45,
		47,
		50,
		54,
		56,
		125,
		126
	};

	/** Second profession classes (level 40 cases). */
	private static final int[] CLASSES_L40 =
	{
		2,
		3,
		5,
		8,
		9,
		12,
		16,
		20,
		23,
		24,
		27,
		30,
		33,
		36,
		40,
		46,
		48,
		51,
		55,
		57,
		127,
		130
	};

	/** Third profession classes (level 52, 76 and 85 cases). */
	private static final int[] CLASSES_L52_UP =
	{
		88,
		89,
		90,
		92,
		93,
		94,
		97,
		99,
		102,
		103,
		105,
		108,
		113,
		115,
		118,
		131,
		134,
		136
	};

	private static List<Spec> nakedSpecs()
	{
		final List<Spec> list = new ArrayList<>();
		for (int classId : STARTING_CLASSES)
		{
			for (boolean female : new boolean[]
			{
				false,
				true
			})
			{
				for (boolean skills : new boolean[]
				{
					false,
					true
				})
				{
					list.add(Spec.player("naked_c" + classId + "_l1_" + (female ? "f" : "m") + (skills ? "_skills" : "_noskills"), classId, female, 1, new int[0][], skills));
				}
			}
		}

		addLevel(list, 20, CLASSES_L20);
		addLevel(list, 40, CLASSES_L40);
		addLevel(list, 52, CLASSES_L52_UP);
		addLevel(list, 76, CLASSES_L52_UP);
		addLevel(list, 85, CLASSES_L52_UP);
		return list;
	}

	private static void addLevel(List<Spec> list, int level, int[] classes)
	{
		for (int classId : classes)
		{
			list.add(Spec.player("naked_c" + classId + "_l" + level + "_m_noskills", classId, false, level, new int[0][], false));
			list.add(Spec.player("naked_c" + classId + "_l" + level + "_m_skills", classId, false, level, new int[0][], true));
		}
	}

	private static List<Spec> gearSpecs()
	{
		final List<Spec> list = new ArrayList<>();
		final Object[][] combos =
		{
			// name, gear, class id, level
			{
				"heavy",
				GEAR_HEAVY_A,
				88,
				76
			},
			{
				"heavy",
				GEAR_HEAVY_A,
				2,
				40
			},
			{
				"heavy",
				GEAR_HEAVY_A,
				90,
				85
			},
			{
				"lightbow",
				GEAR_LIGHT_BOW_A,
				92,
				76
			},
			{
				"lightbow",
				GEAR_LIGHT_BOW_A,
				9,
				40
			},
			{
				"robe",
				GEAR_ROBE_A,
				94,
				76
			},
			{
				"robe",
				GEAR_ROBE_A,
				12,
				40
			},
			{
				"dagger",
				GEAR_LIGHT_DAGGER_A,
				93,
				85
			},
		};
		for (Object[] combo : combos)
		{
			for (int enchant : new int[]
			{
				0,
				3,
				6,
				16
			})
			{
				for (boolean skills : new boolean[]
				{
					false,
					true
				})
				{
					list.add(Spec.player("gear_" + combo[0] + "_c" + combo[2] + "_l" + combo[3] + "_e" + enchant + (skills ? "_skills" : "_noskills"), (Integer) combo[2], false, (Integer) combo[3], enchanted((int[][]) combo[1], enchant), skills));
				}
			}
		}

		return list;
	}

	/** 30 NPCs: monsters at many levels (melee and archers, aggressive and not), raid and grand bosses, guards (31033 included), gatekeepers, merchant, trainer, village master, folk. */
	public static final int[] NPC_IDS =
	{
		20001,
		20003,
		20006,
		20013,
		20017,
		20103,
		20120,
		20201,
		20239,
		20677,
		20780,
		21001,
		21090,
		21166,
		21321,
		22100,
		18120,
		18366,
		18597,
		25001,
		25010,
		25051,
		25125,
		25283,
		29001,
		29019,
		29028,
		30039,
		31032,
		31033,
		30006,
		30080,
		30001,
		30300,
		31314,
		32030
	};

	/** Every spec of the three stats_*.json files, in file order (used by the regeneration vectors). */
	public static List<Spec> allSpecs()
	{
		final List<Spec> all = new ArrayList<>(nakedSpecs());
		all.addAll(gearSpecs());
		all.addAll(npcSpecs());
		return all;
	}

	private static List<Spec> npcSpecs()
	{
		final List<Spec> list = new ArrayList<>();
		for (int id : NPC_IDS)
		{
			list.add(Spec.npc("npc_" + id, id));
		}

		return list;
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Generation
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	private static void writeCases(Path file, String source, String note, List<Spec> specs, Map<String, Object> extraHeader) throws Exception
	{
		final Map<String, Object> root = Json.header(source);
		root.put("note", note);
		root.putAll(extraHeader);
		final List<Object> cases = new ArrayList<>();
		final List<Object> skipped = new ArrayList<>();
		for (Spec spec : specs)
		{
			final Built built = build(spec);
			if (built == null)
			{
				final Map<String, Object> s = Json.obj();
				s.put("id", spec.id);
				s.put("reason", "no template");
				skipped.add(s);
				continue;
			}

			cases.add(caseOf(built));
		}

		root.put("cases", cases);
		root.put("skipped", skipped);
		Json.write(file, root);
		System.out.println("stats: wrote " + file.getFileName() + " (" + cases.size() + " cases, " + skipped.size() + " skipped)");
	}

	public static void generate(Path outDir) throws Exception
	{
		Bootstrap.init();

		final Map<String, Object> config = Json.obj();
		config.put("AutoLearnSkills", PlayerConfig.AUTO_LEARN_SKILLS);
		config.put("ExpertisePenalty", PlayerConfig.EXPERTISE_PENALTY);

		final String note = "Each case: input (what to build) and expected (user_info = the ints the packet carries, raw = the full doubles and the probes). Players are built like Player.restore + rewardSkills: "
			+ "gear equipped first (Inventory.equipItem, armor set and item skills run), then the auto-get skills (repeated until every skill is at its highest level), then refreshExpertisePenalty and refreshOverloaded. "
			+ "input.skills = the auto-get skills in the order their functions stay in the calculators; input.derived_skills = skills the player knows on top of them (armor set, item, grade penalty). "
			+ "probes = Creature.calcStat(stat, init) with the init value the formulas use, key 'stat@init'. Doubles are Java's shortest round-trip text; read run/walk speeds and p_atk as f64.";
		final Map<String, Object> header = Json.obj();
		header.put("config", config);

		writeCases(outDir.resolve("stats_players_naked.json"), "UserInfo values: CreatureStat/PlayerStat + Calculator + Func* (naked players)", note, nakedSpecs(), header);
		writeCases(outDir.resolve("stats_players_gear.json"), "UserInfo values: CreatureStat/PlayerStat + Calculator + Func* + item funcs + armor sets (geared players)", note, gearSpecs(), header);
		writeCases(outDir.resolve("stats_npcs.json"), "NpcInfo values: CreatureStat/NpcStat + Calculator + Func* (NPCs as spawned)", note, npcSpecs(), header);
		writeBaseBonus(outDir.resolve("stats_base_bonus.json"));
		writeFuncTrace(outDir.resolve("stats_func_trace.json"));

		System.out.println("stats: database access answered by the harness fake: " + DatabaseFactory.connections() + " fake connections, " + DatabaseFactory.statements().size() + " distinct statements");
	}

	/** Creatures whose calculators are dumped (a debugging aid for the Rust stat engine: which function sits where, with which order and value). */
	private static List<Spec> traceSpecs()
	{
		final List<Spec> list = new ArrayList<>();
		list.add(Spec.player("trace_newbie_c0_l1", 0, false, 1, new int[0][], false));
		list.add(Spec.player("trace_naked_c88_l76_skills", 88, false, 76, new int[0][], true));
		list.add(Spec.player("trace_heavy_c88_l76_e6_skills", 88, false, 76, enchanted(GEAR_HEAVY_A, 6), true));
		list.add(Spec.player("trace_heavy_c88_l76_e16_skills", 88, false, 76, enchanted(GEAR_HEAVY_A, 16), true));
		list.add(Spec.player("trace_robe_c94_l76_e3_skills", 94, false, 76, enchanted(GEAR_ROBE_A, 3), true));
		list.add(Spec.player("trace_bow_c92_l76_e0_skills", 92, false, 76, enchanted(GEAR_LIGHT_BOW_A, 0), true));
		list.add(Spec.npc("trace_npc_20677", 20677));
		list.add(Spec.npc("trace_npc_31033", 31033));
		list.add(Spec.npc("trace_npc_25010", 25010));
		return list;
	}

	private static String ownerName(Object owner, Map<Object, String> effectOwners)
	{
		if (owner == null)
		{
			return "null";
		}
		if (owner instanceof org.l2jmobius.gameserver.entity.item.instance.Item item)
		{
			return "item:" + item.getId();
		}
		if (owner instanceof Skill skill)
		{
			return "skill:" + skill.getId() + ":" + skill.getLevel();
		}
		final String effect = effectOwners.get(owner);
		return effect != null ? effect : owner.getClass().getSimpleName();
	}

	private static void writeFuncTrace(Path file) throws Exception
	{
		final Map<String, Object> root = Json.header("Creature.getCalculators(): the function lists behind the numbers");
		root.put("note", "Per creature and stat, the functions of its Calculator in evaluation order (Calculator.addFunc keeps them sorted by order, equal orders in insertion order). "
			+ "class = the Func* class, owner = item:<id> (item stat functions), skill:<id>:<level> (functions of the skill itself), skill:<id>:<level>/<Effect> (functions made by a passive skill effect), or null (the standard formula functions every creature gets), "
			+ "cond = class of the apply condition or null. Debugging aid for the stat engine; the numbers to match are in the stats_*.json files.");
		final List<Object> cases = new ArrayList<>();
		for (Spec spec : traceSpecs())
		{
			final Built built = build(spec);
			final Map<String, Object> c = Json.obj();
			c.put("id", spec.id);
			c.put("input", input(built));
			final Map<String, Object> stats = Json.obj();
			// Passive skills make their functions through their effects: map each effect object back to its skill.
			final Map<Object, String> effectOwners = new java.util.IdentityHashMap<>();
			for (Skill known : built.creature.getSkills().values())
			{
				for (org.l2jmobius.gameserver.mechanics.skill.EffectScope scope : org.l2jmobius.gameserver.mechanics.skill.EffectScope.values())
				{
					final List<org.l2jmobius.gameserver.mechanics.effects.AbstractEffect> effects = known.getEffects(scope);
					if (effects != null)
					{
						for (org.l2jmobius.gameserver.mechanics.effects.AbstractEffect effect : effects)
						{
							effectOwners.put(effect, "skill:" + known.getId() + ":" + known.getLevel() + "/" + effect.getClass().getSimpleName());
						}
					}
				}
			}

			final org.l2jmobius.gameserver.mechanics.stats.Calculator[] calculators = built.creature.getCalculators();
			for (int i = 0; i < calculators.length; i++)
			{
				if ((calculators[i] == null) || (calculators[i].size() == 0))
				{
					continue;
				}

				final List<Object> funcs = new ArrayList<>();
				for (org.l2jmobius.gameserver.mechanics.stats.functions.AbstractFunction func : calculators[i].getFunctions())
				{
					final Map<String, Object> f = Json.obj();
					f.put("class", func.getClass().getSimpleName());
					f.put("order", func.getOrder());
					f.put("value", func.getValue());
					f.put("owner", ownerName(func.getFuncOwner(), effectOwners));
					f.put("cond", func.getApplyCond() == null ? null : func.getApplyCond().getClass().getSimpleName());
					funcs.add(Json.compact(f));
				}

				stats.put(Stat.values()[i].name(), funcs);
			}

			c.put("calculators", stats);
			cases.add(c);
		}

		root.put("cases", cases);
		Json.write(file, root);
		System.out.println("stats: wrote " + file.getFileName() + " (" + cases.size() + " creatures)");
	}

	private static void writeBaseBonus(Path file) throws Exception
	{
		final Map<String, Object> root = Json.header("BaseStat (statBonus.xml): STRbonus, INTbonus, DEXbonus, WITbonus, CONbonus, MENbonus");
		root.put("note", "Index = the stat value (0..99); BaseStat.X.calcBonus(creature) is table[creature.getX()].");
		for (String name : new String[]
		{
			"STR",
			"INT",
			"DEX",
			"WIT",
			"CON",
			"MEN"
		})
		{
			final Field field = BaseStat.class.getDeclaredField(name + "bonus");
			field.setAccessible(true);
			final double[] table = (double[]) field.get(null);
			final List<Object> list = new ArrayList<>();
			for (double v : table)
			{
				list.add(v);
			}

			root.put(name.toLowerCase(), Json.compact(list));
		}

		Json.write(file, root);
		System.out.println("stats: wrote " + file.getFileName());
	}
}
