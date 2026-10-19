package l2rust.vectors;

import java.lang.reflect.Field;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.TreeMap;
import java.util.TreeSet;

import org.l2jmobius.commons.database.DatabaseFactory;
import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.config.RatesConfig;
import org.l2jmobius.gameserver.config.custom.PremiumSystemConfig;
import org.l2jmobius.gameserver.data.xml.ItemData;
import org.l2jmobius.gameserver.entity.actor.Npc;
import org.l2jmobius.gameserver.entity.actor.Player;
import org.l2jmobius.gameserver.entity.actor.enums.npc.DropType;
import org.l2jmobius.gameserver.entity.actor.holders.npc.DropGroupHolder;
import org.l2jmobius.gameserver.entity.actor.holders.npc.DropHolder;
import org.l2jmobius.gameserver.entity.actor.templates.NpcTemplate;
import org.l2jmobius.gameserver.entity.item.holders.ItemHolder;
import org.l2jmobius.gameserver.mechanics.stats.Stat;
import org.l2jmobius.gameserver.mechanics.stats.functions.FuncAdd;

/**
 * Drop vectors (M4-JV-02): the real {@code NpcTemplate.calculateDrops(DropType.DROP, victim, killer)} of L2JMobius (group lists, ungrouped lists, the level gap, the drop occurrence
 * counter, the rate multipliers and the bonus stats of the killer) with every {@code Rnd} draw scripted and recorded, for real monsters and raid bosses of the datapack.
 * <p>
 * Files (all in the output directory):
 * <ul>
 * <li>{@code drops_setup.json}: the NPC drop lists as the loader left them (the input of every case), the items they use, the rate profiles, the scripts.</li>
 * <li>{@code drops_monsters.json}: monsters, shipped {@code Rates.ini}, level gaps 0/5/9/10/15 (and some extra).</li>
 * <li>{@code drops_raids.json}: raid bosses and grand bosses, shipped {@code Rates.ini}.</li>
 * <li>{@code drops_rates.json}: custom rate profiles (multipliers, per item maps, occurrence limits, level gap settings, bonus stats of the killer).</li>
 * </ul>
 * Needs the game server root as the working directory (see {@link Bootstrap}). The killer is a real {@link Player} (class 0) of the wanted level; the bonus killer has
 * {@code FuncAdd} functions on the four bonus drop stats, so {@code PlayerStat.getBonus*Multiplier()} returns the values the case records.
 * <p>
 * Out of scope: spoil (M5), parties, champions, premium (the generator checks it is off), seeded monsters, {@code BossDropEnable} (applied when the NPC data loads).
 */
public final class DropVectors
{
	private DropVectors()
	{
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// What is rolled
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	/** Monsters with group lists, spread over the levels (herbs, 100 % items, adena in a group). */
	private static final int[] MONSTER_GROUPS =
	{
		20432, // lv 1
		20093, // lv 10
		20100, // lv 13
		20103, // lv 15, herbs
		18478, // lv 15, herbs, no adena
		20048, // lv 20
		20077, // lv 30, herbs
		20201, // lv 32, herbs
		20135, // lv 40
		18544, // lv 45, six groups, no adena
		20239, // lv 49
		21001, // lv 55
		20651, // lv 60
		20677, // lv 66
		20629, // lv 70
		18579, // lv 70, herbs, no adena
		21090 // lv 75
	};

	/** Monsters with an ungrouped list, and with both kinds. */
	private static final int[] MONSTER_OTHER =
	{
		18352, // lv 22, one ungrouped 100 % item
		22809, // lv 73, seven ungrouped items with adena
		22810, // lv 72
		22320, // lv 81, three groups + one ungrouped item
		22329 // lv 81
	};

	/** Raid bosses (ungrouped lists, 7 to 28 items), grand bosses and raid bosses with groups. */
	private static final int[] RAIDS =
	{
		25001, // lv 23, 12 items
		25010, // lv 55, 14 items
		25044, // lv 47, 22 items
		25064, // lv 40, 28 items
		25035, // lv 70
		25054, // lv 75
		25512, // grand boss lv 52, 20 ungrouped items
		29001, // grand boss lv 40, 13 groups
		29006, // grand boss lv 50, 10 groups
		29022, // grand boss lv 60, 17 groups
		29019, // grand boss lv 79, 32 groups + ungrouped items
		29020, // grand boss lv 75, 4 groups + a 100 % ungrouped item
		29030 // raid boss lv 84, one group
	};

	/** The NPCs of the custom rate profiles. */
	private static final int[] RATE_NPCS =
	{
		20103,
		20239,
		18544,
		20677,
		18352,
		22320,
		25010,
		25064,
		29001,
		29019
	};

	/** Killer level minus victim level. Item drops fall to the minimum chance at 10 and adena at 15. */
	private static final int[] GAPS =
	{
		0,
		5,
		9,
		10,
		15
	};

	/** More gaps for some NPCs: the victim is 5 above the killer, and the adena range (8 to 15). */
	private static final int[] EXTRA_GAPS =
	{
		-5,
		8,
		12
	};

	private static final int[] EXTRA_GAP_NPCS =
	{
		20103,
		20239,
		22320,
		25010
	};

	private static final int MAX_PLAYER_LEVEL = 85;

	/** name -> description of the scripted draws. Each script is long enough for the biggest drop list. */
	private static final Map<String, String> SCRIPTS = new LinkedHashMap<>();

	static
	{
		SCRIPTS.put("zero", "every draw 0.0: every level gap check passes, every chance succeeds, every count is the minimum");
		SCRIPTS.put("high", "every draw 0.9999: level gap checks pass only at 100 %, only chances of 100 % or more succeed, every count is the maximum");
		SCRIPTS.put("seed1", "java.util.Random(1).nextDouble() x 600, uniform");
		SCRIPTS.put("cube2", "u*u*u of java.util.Random(2).nextDouble() x 600: biased low, many drops succeed");
		SCRIPTS.put("cube3", "u*u*u of java.util.Random(3).nextDouble() x 600: biased low");
	}

	private static final int SCRIPT_LENGTH = 600;

	private static double[] script(String name)
	{
		switch (name)
		{
			case "zero":
				return constant(0.0);
			case "high":
				return constant(0.9999);
			case "seed1":
				return seeded(1, false);
			case "cube2":
				return seeded(2, true);
			case "cube3":
				return seeded(3, true);
			default:
				throw new IllegalArgumentException(name);
		}
	}

	private static double[] constant(double u)
	{
		final double[] s = new double[SCRIPT_LENGTH];
		Arrays.fill(s, u);
		return s;
	}

	private static double[] seeded(long seed, boolean cube)
	{
		final Random random = new Random(seed);
		final double[] s = new double[SCRIPT_LENGTH];
		for (int i = 0; i < s.length; i++)
		{
			final double u = random.nextDouble();
			s[i] = cube ? u * u * u : u;
		}

		return s;
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Rate profiles: values of RatesConfig set before the case and restored after
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	private static final String[] RATE_FIELDS =
	{
		"RATE_DEATH_DROP_AMOUNT_MULTIPLIER",
		"RATE_SPOIL_DROP_AMOUNT_MULTIPLIER",
		"RATE_HERB_DROP_AMOUNT_MULTIPLIER",
		"RATE_RAID_DROP_AMOUNT_MULTIPLIER",
		"RATE_DEATH_DROP_CHANCE_MULTIPLIER",
		"RATE_SPOIL_DROP_CHANCE_MULTIPLIER",
		"RATE_HERB_DROP_CHANCE_MULTIPLIER",
		"RATE_RAID_DROP_CHANCE_MULTIPLIER",
		"RATE_DROP_AMOUNT_BY_ID",
		"RATE_DROP_CHANCE_BY_ID",
		"DROP_MAX_OCCURRENCES_NORMAL",
		"DROP_MAX_OCCURRENCES_RAIDBOSS",
		"DROP_ADENA_MIN_LEVEL_DIFFERENCE",
		"DROP_ADENA_MAX_LEVEL_DIFFERENCE",
		"DROP_ADENA_MIN_LEVEL_GAP_CHANCE",
		"DROP_ITEM_MIN_LEVEL_DIFFERENCE",
		"DROP_ITEM_MAX_LEVEL_DIFFERENCE",
		"DROP_ITEM_MIN_LEVEL_GAP_CHANCE"
	};

	private static Map<String, Object> snapshot() throws Exception
	{
		final Map<String, Object> snap = new LinkedHashMap<>();
		for (String name : RATE_FIELDS)
		{
			snap.put(name, RatesConfig.class.getField(name).get(null));
		}

		return snap;
	}

	private static void restore(Map<String, Object> snap) throws Exception
	{
		for (Map.Entry<String, Object> e : snap.entrySet())
		{
			RatesConfig.class.getField(e.getKey()).set(null, e.getValue());
		}
	}

	private static Map<Integer, Float> map(Object... idAndValue)
	{
		final Map<Integer, Float> m = new HashMap<>();
		for (int i = 0; i < idAndValue.length; i += 2)
		{
			m.put((Integer) idAndValue[i], (Float) idAndValue[i + 1]);
		}

		return m;
	}

	/** A rate profile. {@code apply} sets the RatesConfig fields, {@code bonus} says the killer has the bonus drop stats. */
	private static final class Profile
	{
		final String name;
		final String note;
		final boolean bonus;
		final Runnable apply;

		Profile(String name, String note, boolean bonus, Runnable apply)
		{
			this.name = name;
			this.note = note;
			this.bonus = bonus;
			this.apply = apply;
		}
	}

	private static final Profile SHIPPED = new Profile("shipped", "Rates.ini as shipped (all multipliers 1, the per item maps 57,1;... and 21747,0;...)", false, () ->
	{
	});

	private static void x1Defaults()
	{
		RatesConfig.RATE_DEATH_DROP_AMOUNT_MULTIPLIER = 1f;
		RatesConfig.RATE_SPOIL_DROP_AMOUNT_MULTIPLIER = 1f;
		RatesConfig.RATE_HERB_DROP_AMOUNT_MULTIPLIER = 1f;
		RatesConfig.RATE_RAID_DROP_AMOUNT_MULTIPLIER = 1f;
		RatesConfig.RATE_DEATH_DROP_CHANCE_MULTIPLIER = 1f;
		RatesConfig.RATE_SPOIL_DROP_CHANCE_MULTIPLIER = 1f;
		RatesConfig.RATE_HERB_DROP_CHANCE_MULTIPLIER = 1f;
		RatesConfig.RATE_RAID_DROP_CHANCE_MULTIPLIER = 1f;
		RatesConfig.RATE_DROP_AMOUNT_BY_ID = new HashMap<>();
		RatesConfig.RATE_DROP_CHANCE_BY_ID = new HashMap<>();
	}

	private static final Profile[] CUSTOM_PROFILES =
	{
		new Profile("x1_empty_maps", "all multipliers 1 and no per item map (the Java defaults)", false, DropVectors::x1Defaults),
		new Profile("rates_x5", "death chance x5 amount x3, raid chance x2 amount x4, herb chance x3 amount x2, no per item map", false, () ->
		{
			x1Defaults();
			RatesConfig.RATE_DEATH_DROP_CHANCE_MULTIPLIER = 5f;
			RatesConfig.RATE_DEATH_DROP_AMOUNT_MULTIPLIER = 3f;
			RatesConfig.RATE_RAID_DROP_CHANCE_MULTIPLIER = 2f;
			RatesConfig.RATE_RAID_DROP_AMOUNT_MULTIPLIER = 4f;
			RatesConfig.RATE_HERB_DROP_CHANCE_MULTIPLIER = 3f;
			RatesConfig.RATE_HERB_DROP_AMOUNT_MULTIPLIER = 2f;
		}),
		new Profile("rates_fractional", "chance 0.1 / 0.3 / 0.7 and amount 2.5 / 1.5 / 0.5 (a Java float widened to double is not the decimal number)", false, () ->
		{
			x1Defaults();
			RatesConfig.RATE_DEATH_DROP_CHANCE_MULTIPLIER = 0.1f;
			RatesConfig.RATE_DEATH_DROP_AMOUNT_MULTIPLIER = 2.5f;
			RatesConfig.RATE_RAID_DROP_CHANCE_MULTIPLIER = 0.3f;
			RatesConfig.RATE_RAID_DROP_AMOUNT_MULTIPLIER = 1.5f;
			RatesConfig.RATE_HERB_DROP_CHANCE_MULTIPLIER = 0.7f;
			RatesConfig.RATE_HERB_DROP_AMOUNT_MULTIPLIER = 0.5f;
		}),
		new Profile("by_id_high", "per item maps: adena chance 200 (capped to 100) amount 3.5, herb 8600 chance 2.5 amount 2, 1867 chance 4 amount 1.25, 17 amount 0.5; death chance x2", false, () ->
		{
			x1Defaults();
			RatesConfig.RATE_DEATH_DROP_CHANCE_MULTIPLIER = 2f;
			RatesConfig.RATE_DROP_CHANCE_BY_ID = map(57, 200f, 8600, 2.5f, 1867, 4f);
			RatesConfig.RATE_DROP_AMOUNT_BY_ID = map(57, 3.5f, 8600, 2f, 1867, 1.25f, 17, 0.5f);
		}),
		new Profile("by_id_low", "per item maps: adena chance 0.5, herb 8600 chance 0 (never drops), 1867 chance 1 (exactly 1: the x1 path), amount of 57 0.25", false, () ->
		{
			x1Defaults();
			RatesConfig.RATE_DROP_CHANCE_BY_ID = map(57, 0.5f, 8600, 0f, 1867, 1f);
			RatesConfig.RATE_DROP_AMOUNT_BY_ID = map(57, 0.25f);
		}),
		new Profile("occurrences_1_3", "shipped rates, at most 1 item (3 for raids) below 100 % chance", false, () ->
		{
			RatesConfig.DROP_MAX_OCCURRENCES_NORMAL = 1;
			RatesConfig.DROP_MAX_OCCURRENCES_RAIDBOSS = 3;
		}),
		new Profile("occurrences_0", "shipped rates, DropMaxOccurrences 0 (the counter is 0 before the first item: nothing is rolled)", false, () ->
		{
			RatesConfig.DROP_MAX_OCCURRENCES_NORMAL = 0;
			RatesConfig.DROP_MAX_OCCURRENCES_RAIDBOSS = 0;
		}),
		new Profile("occurrences_5_20", "shipped rates, at most 5 items (20 for raids)", false, () ->
		{
			RatesConfig.DROP_MAX_OCCURRENCES_NORMAL = 5;
			RatesConfig.DROP_MAX_OCCURRENCES_RAIDBOSS = 20;
		}),
		new Profile("occurrences_1_x5", "rates_x5 with at most 1 item (3 for raids): the counter logic with custom rates", false, () ->
		{
			x1Defaults();
			RatesConfig.RATE_DEATH_DROP_CHANCE_MULTIPLIER = 5f;
			RatesConfig.RATE_RAID_DROP_CHANCE_MULTIPLIER = 2f;
			RatesConfig.DROP_MAX_OCCURRENCES_NORMAL = 1;
			RatesConfig.DROP_MAX_OCCURRENCES_RAIDBOSS = 3;
		}),
		new Profile("gaps_custom", "shipped rates, adena gap 3..6 down to 30 %, item gap 2..4 down to 50 %", false, () ->
		{
			RatesConfig.DROP_ADENA_MIN_LEVEL_DIFFERENCE = 3;
			RatesConfig.DROP_ADENA_MAX_LEVEL_DIFFERENCE = 6;
			RatesConfig.DROP_ADENA_MIN_LEVEL_GAP_CHANCE = 30;
			RatesConfig.DROP_ITEM_MIN_LEVEL_DIFFERENCE = 2;
			RatesConfig.DROP_ITEM_MAX_LEVEL_DIFFERENCE = 4;
			RatesConfig.DROP_ITEM_MIN_LEVEL_GAP_CHANCE = 50;
		}),
		new Profile("gaps_equal", "shipped rates, min = max level difference (8 and 5): scaleToRange divides 0 by 0, the gap chance is NaN and no gap check ever rejects", false, () ->
		{
			RatesConfig.DROP_ADENA_MAX_LEVEL_DIFFERENCE = 8;
			RatesConfig.DROP_ITEM_MAX_LEVEL_DIFFERENCE = 5;
		}),
		new Profile("bonus_killer", "shipped rates, the killer has bonus drop rate +35 %, amount +20 %, adena +60 %", true, () ->
		{
		}),
		new Profile("bonus_killer_x5", "rates_x5 and the killer with bonus drop rate +35 %, amount +20 %, adena +60 %", true, () ->
		{
			x1Defaults();
			RatesConfig.RATE_DEATH_DROP_CHANCE_MULTIPLIER = 5f;
			RatesConfig.RATE_DEATH_DROP_AMOUNT_MULTIPLIER = 3f;
			RatesConfig.RATE_RAID_DROP_CHANCE_MULTIPLIER = 2f;
			RatesConfig.RATE_RAID_DROP_AMOUNT_MULTIPLIER = 4f;
		})
	};

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Killers and victims
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	private static final Map<Integer, Player> PLAIN_KILLERS = new HashMap<>();
	private static final Map<Integer, Player> BONUS_KILLERS = new HashMap<>();

	private static Player killer(int level, boolean bonus) throws Exception
	{
		final Map<Integer, Player> cache = bonus ? BONUS_KILLERS : PLAIN_KILLERS;
		Player player = cache.get(level);
		if (player == null)
		{
			player = Bootstrap.newPlayer(0, false, level);
			if (bonus)
			{
				player.addStatFunc(new FuncAdd(Stat.BONUS_DROP_RATE, 0x10, "drop-vectors", 35, null));
				player.addStatFunc(new FuncAdd(Stat.BONUS_DROP_AMOUNT, 0x10, "drop-vectors", 20, null));
				player.addStatFunc(new FuncAdd(Stat.BONUS_DROP_ADENA, 0x10, "drop-vectors", 60, null));
				player.addStatFunc(new FuncAdd(Stat.BONUS_SPOIL_RATE, 0x10, "drop-vectors", 10, null));
			}

			cache.put(level, player);
		}

		return player;
	}

	private static final Map<Integer, Npc> NPCS = new HashMap<>();

	private static Npc npc(int id) throws Exception
	{
		Npc npc = NPCS.get(id);
		if (npc == null)
		{
			npc = Bootstrap.newNpc(id);
			if (npc == null)
			{
				throw new IllegalStateException("no template for NPC " + id);
			}

			NPCS.put(id, npc);
		}

		return npc;
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Output
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	private static final String RND_NOTE = "Every case has 'rnd': every Rnd call the real calculateDrops made, in order, one string per call: 'D<u>' is Rnd.nextDouble() returning u in [0,1); "
		+ "'L<origin>,<bound>=<value>' is Rnd.get(long origin, long bound) returning the value (origin + floor(u*(bound-origin+1)); a call with origin >= bound draws nothing and is not recorded); "
		+ "any other call is '<method> <bound>=<value>' (none occurs in drops). 'script' names the scripted unit values (see drops_setup.json). "
		+ "Replay the returned values in order and check the kind and bounds. 'result' is the list of [item id, count] calculateDrops returned (an empty list is Java's null).";

	private static List<Object> floats(Map<Integer, Float> m)
	{
		final List<Object> list = new ArrayList<>();
		for (Map.Entry<Integer, Float> e : new TreeMap<>(m).entrySet())
		{
			list.add(Json.compact(Arrays.asList((Object) e.getKey(), e.getValue())));
		}

		return list;
	}

	/** The RatesConfig values the drop code reads, as JSON (floats stay floats). */
	@SuppressWarnings("unchecked")
	private static Map<String, Object> settings() throws Exception
	{
		final Map<String, Object> m = Json.obj();
		for (String name : RATE_FIELDS)
		{
			final Object value = RatesConfig.class.getField(name).get(null);
			m.put(name, value instanceof Map ? floats((Map<Integer, Float>) value) : value);
		}

		return m;
	}

	private static Map<String, Object> npcSetup(int id, TreeSet<Integer> itemIds) throws Exception
	{
		final Npc npc = npc(id);
		final NpcTemplate template = npc.getTemplate();
		final Map<String, Object> m = Json.obj();
		m.put("id", id);
		m.put("name", template.getName());
		m.put("type", template.getType());
		m.put("level", npc.getLevel());
		m.put("is_raid", npc.isRaid());
		m.put("is_champion", npc.isChampion());
		final List<Object> groups = new ArrayList<>();
		if (template.getDropGroups() != null)
		{
			for (DropGroupHolder group : template.getDropGroups())
			{
				final Map<String, Object> g = Json.obj();
				g.put("chance", group.getChance());
				final List<Object> items = new ArrayList<>();
				for (DropHolder drop : group.getDropList())
				{
					items.add(Json.compact(dropItem(drop)));
					itemIds.add(drop.getItemId());
				}

				g.put("items", items);
				groups.add(g);
			}
		}

		m.put("drop_groups", groups);
		final List<Object> drops = new ArrayList<>();
		if (template.getDropList() != null)
		{
			for (DropHolder drop : template.getDropList())
			{
				drops.add(Json.compact(dropItem(drop)));
				itemIds.add(drop.getItemId());
			}
		}

		m.put("drops", drops);
		return m;
	}

	private static List<Object> dropItem(DropHolder drop)
	{
		return Arrays.asList((Object) drop.getItemId(), drop.getMin(), drop.getMax(), drop.getChance());
	}

	private static final class Out
	{
		final List<Object> cases = new ArrayList<>();
		int draws = 0;
		int dropped = 0;
		int empty = 0;
	}

	private static void rollCase(Out out, String kind, int npcId, Profile profile, int gap, String scriptName) throws Exception
	{
		final Npc npc = npc(npcId);
		final int killerLevel = npc.getLevel() + gap;
		if ((killerLevel < 1) || (killerLevel > MAX_PLAYER_LEVEL))
		{
			return;
		}

		final Player killer = killer(killerLevel, profile.bonus);
		final NpcTemplate template = npc.getTemplate();
		final double[] script = script(scriptName);
		Rnd.arm(script);
		final List<ItemHolder> result;
		final List<Rnd.Call> calls;
		try
		{
			result = template.calculateDrops(DropType.DROP, npc, killer);
		}
		finally
		{
			calls = Rnd.disarm();
		}

		final Map<String, Object> c = Json.obj();
		c.put("id", kind + "_" + npcId + "_" + profile.name + "_g" + gap + "_" + scriptName);
		c.put("npc", npcId);
		c.put("profile", profile.name);
		c.put("level_gap", gap);
		final Map<String, Object> k = Json.obj();
		k.put("level", killer.getLevel());
		k.put("bonus_drop_rate", killer.getStat().getBonusDropRateMultiplier());
		k.put("bonus_drop_amount", killer.getStat().getBonusDropAmountMultiplier());
		k.put("bonus_drop_adena", killer.getStat().getBonusDropAdenaMultiplier());
		k.put("bonus_spoil_rate", killer.getStat().getBonusSpoilRateMultiplier());
		c.put("killer", Json.compact(k));
		c.put("script", scriptName);
		// One short string per draw (60 000 draws would be 4 MB as objects): "D<u>" for nextDouble, "L<origin>,<bound>=<value>" for get(long, long), else "<method> <bound>=<value>".
		final List<Object> rnd = new ArrayList<>();
		for (Rnd.Call call : calls)
		{
			switch (call.method)
			{
				case "nextDouble" -> rnd.add("D" + call.returned);
				case "get_long_range" -> rnd.add("L" + call.bound + "=" + call.returned);
				default -> rnd.add(call.method + " " + call.bound + "=" + call.returned);
			}
		}

		c.put("rnd", Json.compact(rnd));
		final List<Object> items = new ArrayList<>();
		if (result != null)
		{
			for (ItemHolder holder : result)
			{
				items.add(Json.compact(Arrays.asList((Object) holder.getId(), holder.getCount())));
			}
		}

		c.put("result", items);
		out.cases.add(c);
		out.draws += calls.size();
		out.dropped += items.size();
		if (items.isEmpty())
		{
			out.empty++;
		}
	}

	private static boolean has(int[] ids, int id)
	{
		for (int i : ids)
		{
			if (i == id)
			{
				return true;
			}
		}

		return false;
	}

	private static void write(Path dir, String file, String source, String note, List<Object> cases) throws Exception
	{
		final Map<String, Object> root = Json.header(source);
		root.put("note", note + " " + RND_NOTE);
		root.put("setup_file", "drops_setup.json");
		root.put("cases", cases);
		Json.write(dir.resolve(file), root);
		System.out.println("drops: wrote " + file + " (" + cases.size() + " cases)");
	}

	public static void generate(Path outDir) throws Exception
	{
		Bootstrap.init();
		if (PremiumSystemConfig.PREMIUM_SYSTEM_ENABLED)
		{
			throw new IllegalStateException("the drop vectors are only valid with the premium system off");
		}

		final Map<String, Object> shipped = snapshot();
		final String source = "NpcTemplate.calculateDrops(DropType.DROP, victim, killer) (calculateGroupDrops, calculateUngroupedDrops, calculateGroupDrop, calculateUngroupedDrop)";

		// Setup: the NPCs, their items, the profiles.
		final TreeSet<Integer> itemIds = new TreeSet<>();
		final List<Object> npcSetups = new ArrayList<>();
		final List<Integer> allNpcs = new ArrayList<>();
		for (int[] ids : new int[][]
		{
			MONSTER_GROUPS,
			MONSTER_OTHER,
			RAIDS
		})
		{
			for (int id : ids)
			{
				allNpcs.add(id);
			}
		}

		for (int id : allNpcs)
		{
			npcSetups.add(npcSetup(id, itemIds));
		}

		final List<Object> items = new ArrayList<>();
		for (int id : itemIds)
		{
			final Map<String, Object> item = Json.obj();
			item.put("id", id);
			item.put("ex_immediate_effect", ItemData.getInstance().getTemplate(id).hasExImmediateEffect());
			items.add(Json.compact(item));
		}

		final List<Object> profiles = new ArrayList<>();
		final List<Profile> allProfiles = new ArrayList<>();
		allProfiles.add(SHIPPED);
		allProfiles.addAll(Arrays.asList(CUSTOM_PROFILES));
		for (Profile profile : allProfiles)
		{
			restore(shipped);
			profile.apply.run();
			final Map<String, Object> p = Json.obj();
			p.put("name", profile.name);
			p.put("note", profile.note);
			p.put("bonus_killer", profile.bonus);
			p.put("settings", settings());
			profiles.add(p);
		}

		restore(shipped);

		final Map<String, Object> setup = Json.header(source);
		setup.put("note", "The input of every case in drops_monsters.json, drops_raids.json and drops_rates.json. 'npcs': the drop lists the NPC loader left (NpcData: items that do not exist removed, "
			+ "'drops' sorted by chance high to low, the items of each group sorted low to high, groups in document order), items as [item id, min, max, chance]; is_raid is the victim's isRaid(). "
			+ "'items': every item the lists use and its hasExImmediateEffect(). 'profiles': the RatesConfig values of each rate profile (floats are Java floats, the per item maps are [item id, value]); "
			+ "'bonus_killer' says the killer has the bonus drop stats (the case's 'killer' block has the multipliers PlayerStat returned). 'scripts': the scripted draws.");
		setup.put("npcs", npcSetups);
		setup.put("items", items);
		setup.put("profiles", profiles);
		final Map<String, Object> scripts = Json.obj();
		scripts.putAll(SCRIPTS);
		setup.put("scripts", scripts);
		Json.write(outDir.resolve("drops_setup.json"), setup);
		System.out.println("drops: wrote drops_setup.json (" + npcSetups.size() + " NPCs, " + items.size() + " items, " + profiles.size() + " profiles)");

		// Monsters, shipped rates.
		final Out monsters = new Out();
		for (int[] ids : new int[][]
		{
			MONSTER_GROUPS,
			MONSTER_OTHER
		})
		{
			for (int id : ids)
			{
				rollAll(monsters, "monster", id, SHIPPED);
			}
		}

		write(outDir, "drops_monsters.json", source, "Monsters (group lists, ungrouped lists, both) with the shipped Rates.ini at the level gaps 0/5/9/10/15 (killer level minus victim level), more gaps for 20103, 20239, 22320 and 25010.", monsters.cases);

		// Raid bosses, shipped rates.
		final Out raids = new Out();
		for (int id : RAIDS)
		{
			rollAll(raids, "raid", id, SHIPPED);
		}

		write(outDir, "drops_raids.json", source, "Raid bosses and grand bosses with the shipped Rates.ini (the victim is a raid: DropMaxOccurrencesRaidboss and the raid multipliers).", raids.cases);

		// Custom rates.
		final Out rates = new Out();
		for (Profile profile : CUSTOM_PROFILES)
		{
			restore(shipped);
			profile.apply.run();
			for (int id : RATE_NPCS)
			{
				for (int gap : new int[]
				{
					0,
					10
				})
				{
					for (String scriptName : new String[]
					{
						"zero",
						"seed1",
						"cube3"
					})
					{
						rollCase(rates, "rates", id, profile, gap, scriptName);
					}
				}
			}
		}

		restore(shipped);
		write(outDir, "drops_rates.json", source, "Custom rate profiles (see 'profiles' in drops_setup.json) for ten NPCs at the level gaps 0 and 10.", rates.cases);

		System.out.println("drops: " + (monsters.cases.size() + raids.cases.size() + rates.cases.size()) + " cases, " + (monsters.draws + raids.draws + rates.draws) + " recorded draws, " + (monsters.empty + raids.empty + rates.empty) + " cases without a drop");
		System.out.println("drops: database access answered by the harness fake: " + DatabaseFactory.connections() + " fake connections, " + DatabaseFactory.statements().size() + " distinct statements");
	}

	/** All gaps and scripts of one NPC with the shipped rates ({@code Out} is the file the cases go to). */
	private static void rollAll(Out out, String kind, int id, Profile profile) throws Exception
	{
		for (int gap : GAPS)
		{
			for (String scriptName : SCRIPTS.keySet())
			{
				rollCase(out, kind, id, profile, gap, scriptName);
			}
		}

		if (has(EXTRA_GAP_NPCS, id))
		{
			for (int gap : EXTRA_GAPS)
			{
				for (String scriptName : SCRIPTS.keySet())
				{
					rollCase(out, kind, id, profile, gap, scriptName);
				}
			}
		}
	}
}
