package l2rust.vectors;

import java.lang.reflect.Field;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.Set;
import java.util.TreeSet;
import java.util.function.Supplier;

import org.l2jmobius.commons.database.DatabaseFactory;
import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.config.GeneralConfig;
import org.l2jmobius.gameserver.config.NpcConfig;
import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.config.custom.ClassBalanceConfig;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.entity.Location;
import org.l2jmobius.gameserver.entity.actor.Creature;
import org.l2jmobius.gameserver.entity.item.enums.ShotType;
import org.l2jmobius.gameserver.mechanics.effects.AbstractEffect;
import org.l2jmobius.gameserver.mechanics.skill.EffectScope;
import org.l2jmobius.gameserver.mechanics.skill.Skill;
import org.l2jmobius.gameserver.mechanics.stats.BaseStat;
import org.l2jmobius.gameserver.mechanics.stats.Formulas;
import org.l2jmobius.gameserver.mechanics.stats.Stat;
import org.l2jmobius.gameserver.mechanics.stats.TraitType;
import org.l2jmobius.gameserver.mechanics.stats.functions.FuncAdd;
import org.l2jmobius.gameserver.mechanics.stats.functions.FuncMul;

/**
 * Skill formula vectors (M5-JV-03): the real {@code Formulas} methods that skills use, on the creatures of {@link FormulaVectors} (M4-JV-01) plus a few more and variants with extra stat
 * functions, traits and elements, with every {@code Rnd} draw scripted and recorded. Skills are real skills of the datapack.
 * <p>
 * Files (all in the output directory, all {@code skill_formulas_*.json}):
 * <ul>
 * <li>{@code creatures}: the creatures and variants, with their stats, traits, elements and the stat probes the skill formulas read; the config values</li>
 * <li>{@code skills}: the skills the cases use, with every field the formulas read</li>
 * <li>{@code magic_dam}: {@code calcMagicDam}; {@code mcrit}: {@code calcMCrit} and {@code getMCriticalHit}; {@code mana_dam}: {@code calcManaDam}</li>
 * <li>{@code phys_dam}: {@code calcPhysDam} with a skill; {@code blow}: {@code calcBlowDamage}, {@code calcBackstabDamage}; {@code skill_crit}: {@code calcCrit} with a skill</li>
 * <li>{@code blow_success}: {@code calcBlowSuccess}; {@code phys_evasion}: {@code calcPhysicalSkillEvasion}</li>
 * <li>{@code effect_success}: {@code calcEffectSuccess}; {@code magic_success}: {@code calcMagicSuccess}; {@code magic_affected}: {@code calcMagicAffected}</li>
 * <li>{@code mastery}: {@code calcSkillMastery}, {@code calcEffectAbnormalTime}</li>
 * <li>{@code skill_timing}: {@code calcAtkSpd} for skills, cast and reuse time as {@code Creature.beginCast} computes them</li>
 * <li>{@code resurrect}: {@code calculateSkillResurrectRestorePercent}; {@code heal}: the real {@code Heal.onStart}</li>
 * <li>{@code traits}: {@code calcGeneralTraitBonus}, {@code calcWeaponTraitBonus}, {@code calcAttackTraitBonus}; {@code attribute}: {@code calcAttributeBonus}</li>
 * </ul>
 * Needs the game server root as the working directory (see {@link Bootstrap}).
 */
public final class SkillFormulaVectors
{
	private SkillFormulaVectors()
	{
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Creatures
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	private static final Map<String, StatVectors.Spec> SPECS = new LinkedHashMap<>();
	/** Variant name -> {base creature, mod, mod, ...}. */
	private static final Map<String, String[]> VARIANTS = new LinkedHashMap<>();
	private static final Map<String, StatVectors.Built> BUILT = new LinkedHashMap<>();
	private static final Set<String> SKILLS_USED = new LinkedHashSet<>();

	/** Owner object of the stat functions the variants add. */
	private static final String MOD_OWNER = "skill-formula-vectors";
	/** Order of the added functions: Java's {@code FuncMul} sits at 0x30 and {@code FuncAdd} at 0x40 in the stock stat tables. */
	private static final int ORDER_MUL = 0x30;
	private static final int ORDER_ADD = 0x40;

	static
	{
		// The 17 creatures of formulas_creatures.json (same specs as FormulaVectors) ...
		SPECS.put("duelist76", StatVectors.Spec.player("duelist76", 88, false, 76, StatVectors.enchanted(StatVectors.GEAR_HEAVY_A, 6), true));
		SPECS.put("gladiator40", StatVectors.Spec.player("gladiator40", 2, false, 40, StatVectors.enchanted(StatVectors.GEAR_HEAVY_A, 0), true));
		SPECS.put("archmage76", StatVectors.Spec.player("archmage76", 94, false, 76, StatVectors.enchanted(StatVectors.GEAR_ROBE_A, 3), true));
		SPECS.put("sagittarius76", StatVectors.Spec.player("sagittarius76", 92, false, 76, StatVectors.enchanted(StatVectors.GEAR_LIGHT_BOW_A, 0), true));
		SPECS.put("newbie1", StatVectors.Spec.player("newbie1", 0, false, 1, new int[0][], false));
		SPECS.put("warrior20", StatVectors.Spec.player("warrior20", 1, false, 20, new int[0][], true));
		// ... plus a dagger user (blows) and a healer.
		SPECS.put("adventurer76", StatVectors.Spec.player("adventurer76", 93, false, 76, StatVectors.enchanted(StatVectors.GEAR_LIGHT_DAGGER_A, 0), true));
		SPECS.put("cardinal76", StatVectors.Spec.player("cardinal76", 97, false, 76, StatVectors.enchanted(StatVectors.GEAR_ROBE_A, 0), true));
		for (int npcId : new int[]
		{
			20001,
			20103,
			20201,
			20239,
			20677,
			21001,
			21090,
			18120,
			18366,
			25010,
			31033
		})
		{
			SPECS.put("npc_" + npcId, StatVectors.Spec.npc("npc_" + npcId, npcId));
		}

		// Variants: the base creature with extra stat functions / traits / invulnerabilities. Mod syntax (also written to the creatures file):
		// "add:STAT:v" (FuncAdd, order 0x40), "mul:STAT:v" (FuncMul, order 0x30), "atk_trait:TRAIT:v" / "def_trait:TRAIT:v" (value set, count 1), "invul:TRAIT".
		variant("archmage76_elem", "archmage76", "add:FIRE_POWER:60", "add:WATER_POWER:20", "add:WIND_POWER:30", "add:EARTH_POWER:200", "add:HOLY_POWER:5", "add:DARK_POWER:100");
		variant("archmage76_pvpmods", "archmage76", "mul:PVP_MAGICAL_DMG:1.1", "mul:PVE_MAGICAL_DMG:1.15", "mul:MAGIC_CRIT_DMG:1.3", "add:MCRITICAL_RATE:3");
		variant("archmage76_mastery4", "archmage76", "add:SKILL_MASTERY:4");
		variant("cardinal76_mastery4", "cardinal76", "add:SKILL_MASTERY:4", "mul:SKILL_MASTERY_RATE:2");
		variant("duelist76_mastery1", "duelist76", "add:SKILL_MASTERY:1", "mul:SKILL_MASTERY_RATE:1.5");
		variant("duelist76_skillmods", "duelist76", "mul:PHYSICAL_SKILL_POWER:1.2", "mul:PVP_PHYS_SKILL_DMG:1.1", "mul:PVE_PHYSICAL_DMG:1.1", "mul:CRITICAL_DAMAGE:1.1", "atk_trait:BUG_WEAKNESS:1.25", "atk_trait:ANIMAL_WEAKNESS:1.0", "atk_trait:POISON:1.3", "atk_trait:SLEEP:1.5", "atk_trait:BLEED:0.8", "atk_trait:DEATH:1.2", "atk_trait:SHOCK:1.1", "add:FIRE_POWER:150", "add:WATER_POWER:20", "add:EARTH_POWER:90");
		variant("duelist76_dbi", "duelist76", "add:DEBUFF_IMMUNITY:1");
		variant("duelist76_magres", "duelist76", "mul:MAGIC_SUCCESS_RES:0.8");
		variant("gladiator40_defmods", "gladiator40", "mul:PVP_PHYS_SKILL_DEF:1.2", "mul:PVP_MAGICAL_DEF:1.1", "mul:DEFENCE_CRITICAL_DAMAGE:0.9", "add:P_SKILL_EVASION:30");
		variant("gladiator40_elemres", "gladiator40", "add:FIRE_RES:-100", "add:WIND_RES:200", "add:EARTH_RES:20");
		variant("adventurer76_blow", "adventurer76", "mul:BLOW_RATE:1.1", "add:BLOW_RATE:15");
		variant("npc_20677_res", "npc_20677", "add:FIRE_RES:50", "add:WATER_RES:10", "add:WIND_RES:-30", "add:EARTH_RES:5", "add:HOLY_RES:25", "add:DARK_RES:-40", "mul:MAGIC_SUCCESS_RES:0.8", "def_trait:SWORD:0.8", "def_trait:DAGGER:1.3", "def_trait:BOW:0.9", "def_trait:BUG_WEAKNESS:0.85", "def_trait:ANIMAL_WEAKNESS:1.1", "def_trait:SLEEP:0.4", "def_trait:POISON:0.7", "def_trait:BLEED:1.1", "def_trait:SHOCK:0.5", "def_trait:DEATH:0.5", "def_trait:DERANGEMENT:0.6");
		variant("npc_21090_invul", "npc_21090", "invul:SLEEP", "invul:DEATH", "invul:POISON", "def_trait:POISON:1.0", "add:DEBUFF_VULN:30");
	}

	private static void variant(String name, String base, String... mods)
	{
		final String[] v = new String[mods.length + 1];
		v[0] = base;
		System.arraycopy(mods, 0, v, 1, mods.length);
		VARIANTS.put(name, v);
	}

	private static Creature creature(String name)
	{
		final StatVectors.Built built = BUILT.get(name);
		if (built == null)
		{
			throw new IllegalStateException("unknown creature " + name);
		}

		return built.creature;
	}

	/** Applies one mod string to a fresh variant (see the syntax above). */
	private static void applyMod(Creature c, String mod)
	{
		final String[] p = mod.split(":");
		switch (p[0])
		{
			case "add" -> c.addStatFunc(new FuncAdd(Stat.valueOf(p[1]), ORDER_ADD, MOD_OWNER, Double.parseDouble(p[2]), null));
			case "mul" -> c.addStatFunc(new FuncMul(Stat.valueOf(p[1]), ORDER_MUL, MOD_OWNER, Double.parseDouble(p[2]), null));
			case "atk_trait" ->
			{
				final int i = TraitType.valueOf(p[1]).ordinal();
				c.getStat().getAttackTraits()[i] = Float.parseFloat(p[2]);
				c.getStat().getAttackTraitsCount()[i]++;
			}
			case "def_trait" ->
			{
				final int i = TraitType.valueOf(p[1]).ordinal();
				c.getStat().getDefenceTraits()[i] = Float.parseFloat(p[2]);
				c.getStat().getDefenceTraitsCount()[i]++;
			}
			case "invul" -> c.getStat().getTraitsInvul()[TraitType.valueOf(p[1]).ordinal()]++;
			default -> throw new IllegalArgumentException("bad mod " + mod);
		}
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Skills
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	/** @param key {@code "id:level"} */
	private static Skill skill(String key)
	{
		final String[] p = key.split(":");
		final Skill skill = SkillData.getInstance().getSkill(Integer.parseInt(p[0]), Integer.parseInt(p[1]));
		if ((skill == null) || (skill.getLevel() != Integer.parseInt(p[1])))
		{
			throw new IllegalStateException("skill " + key + " missing");
		}

		SKILLS_USED.add(key);
		return skill;
	}

	private static List<Object> effectNames(Skill skill)
	{
		final List<Object> names = new ArrayList<>();
		for (AbstractEffect e : skill.getEffects(EffectScope.GENERAL))
		{
			names.add(e.getClass().getSimpleName());
		}

		return names;
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Geometry (same as FormulaVectors)
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	private static final int TARGET_X = 100000;
	private static final int TARGET_Y = 100000;
	private static final int TARGET_Z = 0;
	private static final int TARGET_HEADING = 16384;
	private static final Map<String, String> GEOMETRY = new LinkedHashMap<>();

	private static Map<String, Object> geometry()
	{
		final Map<String, Object> g = Json.obj();
		final Map<String, Object> target = Json.obj();
		target.put("x", TARGET_X);
		target.put("y", TARGET_Y);
		target.put("z", TARGET_Z);
		target.put("heading", TARGET_HEADING);
		g.put("target", Json.compact(target));
		final Map<String, Object> attacker = Json.obj();
		for (String relation : new String[]
		{
			"front",
			"side",
			"back"
		})
		{
			final String xy = GEOMETRY.get(relation);
			if (xy != null)
			{
				attacker.put(relation, xy);
			}
		}

		g.put("attacker_xy_by_relation", Json.compact(attacker));
		g.put("note", "The target stands at 'target'. The attacker has heading 0 and z = target z + z_diff, and stands at attacker_xy_by_relation[relation] (found by trying 32 spots 40 units away until the real isInFrontOf / isOnSideOf / isBehind agree). Same spots as the formulas_*.json files.");
		return g;
	}

	private static Map<String, Object> place(Creature attacker, Creature target, String relation, int zDiff)
	{
		final Location t = (Location) target.getLocation();
		t.setXYZ(TARGET_X, TARGET_Y, TARGET_Z);
		t.setHeading(TARGET_HEADING);
		final Location a = (Location) attacker.getLocation();
		for (int step = 0; step < 32; step++)
		{
			final double angle = (Math.PI * 2 * step) / 32;
			a.setXYZ(TARGET_X + (int) Math.round(40 * Math.cos(angle)), TARGET_Y + (int) Math.round(40 * Math.sin(angle)), TARGET_Z + zDiff);
			a.setHeading(0);
			final String actual = attacker.isBehind(target) ? "back" : attacker.isInFrontOf(target) ? "front" : "side";
			if (actual.equals(relation))
			{
				final String xy = a.getX() + "," + a.getY();
				final String known = GEOMETRY.putIfAbsent(relation, xy);
				if ((known != null) && !known.equals(xy))
				{
					throw new IllegalStateException("attacker spot for " + relation + " changed: " + known + " vs " + xy);
				}

				final Map<String, Object> m = Json.obj();
				m.put("relation", relation);
				m.put("z_diff", zDiff);
				return m;
			}
		}

		throw new IllegalStateException("no spot found for relation " + relation);
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Scripted draws
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	private static final class Run
	{
		Object result;
		List<Rnd.Call> calls;
		double[] used;
	}

	private static Run run(double[] script, Supplier<Object> call)
	{
		final Run run = new Run();
		Rnd.arm(script);
		try
		{
			run.result = call.get();
		}
		finally
		{
			final int used = Rnd.used();
			run.calls = Rnd.disarm();
			run.used = Arrays.copyOf(script, used);
		}

		return run;
	}

	private static void putDraws(Map<String, Object> c, Run run)
	{
		final List<Object> script = new ArrayList<>();
		for (double u : run.used)
		{
			script.add(u);
		}

		c.put("script", Json.compact(script));
		final List<Object> calls = new ArrayList<>();
		for (Rnd.Call call : run.calls)
		{
			final Map<String, Object> m = Json.obj();
			m.put("method", call.method);
			m.put("bound", call.bound);
			m.put("returned", call.returned);
			calls.add(Json.compact(m));
		}

		c.put("rnd", Json.compact(calls));
	}

	/** The unit value that makes {@code Rnd.get(100)} return {@code d}. */
	private static double draw100(int d)
	{
		return (Math.max(0, Math.min(99, d)) + 0.5) / 100.0;
	}

	/** The unit value that makes {@code Rnd.get(1000)} return {@code d}. */
	private static double draw1000(int d)
	{
		return (Math.max(0, Math.min(999, d)) + 0.5) / 1000.0;
	}

	/** Pads a script with 0.5 so a path that draws more than the case expects does not run dry (only the consumed values are written). */
	private static double[] script(double... values)
	{
		final double[] s = new double[values.length + 6];
		Arrays.fill(s, 0.5);
		System.arraycopy(values, 0, s, 0, values.length);
		return s;
	}

	private static double[] seeded(long seed, int n)
	{
		final Random random = new Random(seed);
		final double[] s = new double[n];
		for (int i = 0; i < n; i++)
		{
			s[i] = random.nextDouble();
		}

		return s;
	}

	/**
	 * The draws worth testing for a call that compares {@code Rnd.get(100)} against a rate (true while {@code d < rate}): found by calling the real function with all 100 draws.
	 * @return the draws {@code d} (0, 50, 99 and the ones around the threshold), or an empty list when the call draws nothing
	 */
	private static List<Integer> hundredDraws(Supplier<Object> call)
	{
		int trues = 0;
		final boolean[] results = new boolean[100];
		for (int d = 0; d < 100; d++)
		{
			final Run r = run(script(draw100(d)), call);
			if (r.calls.isEmpty())
			{
				return new ArrayList<>();
			}

			results[d] = Boolean.TRUE.equals(r.result);
			if (results[d])
			{
				trues++;
			}
		}

		for (int d = 0; d < 100; d++)
		{
			if (results[d] != (d < trues))
			{
				throw new IllegalStateException("draw result is not monotonic");
			}
		}

		final TreeSet<Integer> draws = new TreeSet<>();
		for (int d : new int[]
		{
			0,
			trues - 1,
			trues,
			trues + 1,
			50,
			99
		})
		{
			draws.add(Math.max(0, Math.min(99, d)));
		}

		return new ArrayList<>(draws);
	}

	/** Runs {@code call} for the draws of {@link #hundredDraws}; calls that draw nothing run once with an empty script. Adds the cases through {@code sink}. */
	private static void forHundredDraws(Supplier<Object> call, DrawSink sink)
	{
		final List<Integer> draws = hundredDraws(call);
		if (draws.isEmpty())
		{
			sink.accept(-1, run(script(), call));
			return;
		}

		for (int d : draws)
		{
			sink.accept(d, run(script(draw100(d)), call));
		}
	}

	private interface DrawSink
	{
		void accept(int draw, Run run);
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Files
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	private static final String RND_NOTE = "Every case has 'script' (the unit values u in [0,1) that were consumed) and 'rnd' (every Rnd call in order: method, bound, returned value; "
		+ "get_int bound b returns floor(u*b), get_int_range o,b returns o+floor(u*(b-o+1)), nextGaussian is 2u-1 in the harness). Calls that draw nothing are not recorded. Replay the returned values in order and check method and bound.";

	private static Map<String, Object> header(String source, String note)
	{
		final Map<String, Object> root = Json.header(source);
		root.put("note", note);
		root.put("creatures_file", "skill_formulas_creatures.json");
		root.put("skills_file", "skill_formulas_skills.json");
		return root;
	}

	private static void write(Path out, String name, Map<String, Object> root, List<Object> cases, boolean withGeometry) throws Exception
	{
		if (withGeometry)
		{
			root.put("geometry", geometry());
		}

		root.put("cases", cases);
		final String file = "skill_formulas_" + name + ".json";
		Json.write(out.resolve(file), root);
		System.out.println("skill-formulas: wrote " + file + " (" + cases.size() + " cases)");
	}

	private static Map<String, Object> pairCase(String id, String attacker, String target)
	{
		final Map<String, Object> c = Json.obj();
		c.put("id", id);
		c.put("attacker", attacker);
		c.put("target", target);
		return c;
	}

	private static void shots(Creature c, String kind)
	{
		c.setChargedShot(ShotType.SOULSHOTS, kind.equals("ss"));
		c.setChargedShot(ShotType.SPIRITSHOTS, kind.equals("sps"));
		c.setChargedShot(ShotType.BLESSED_SPIRITSHOTS, kind.equals("bss"));
	}

	private static Map<String, Object> shotState(Creature c)
	{
		final Map<String, Object> m = Json.obj();
		m.put("ss", c.isChargedShot(ShotType.SOULSHOTS));
		m.put("sps", c.isChargedShot(ShotType.SPIRITSHOTS));
		m.put("bss", c.isChargedShot(ShotType.BLESSED_SPIRITSHOTS));
		return m;
	}

	/** The numbers every damage case reads from the pair. */
	private static Map<String, Object> pairInfo(Creature a, Creature t, Skill s)
	{
		final boolean pvp = a.isPlayable() && t.isPlayable();
		final boolean pve = a.isPlayable() && t.isAttackable();
		final Map<String, Object> m = Json.obj();
		m.put("is_pvp", pvp);
		m.put("is_pve", pve);
		m.put("skill_power", s.getPower(a, t, pvp, pve));
		m.put("p_atk", a.getPAtk(t));
		m.put("p_def", t.getPDef(a));
		m.put("m_atk", a.getMAtk(t, s));
		m.put("m_def", t.getMDef(a, s));
		m.put("shield_def", t.getShldDef());
		m.put("m_critical", a.getMCriticalHit(t, s));
		m.put("attacker_level", (int) a.getLevel());
		m.put("target_level", (int) t.getLevel());
		return m;
	}

	private static String[] pair(String s)
	{
		return s.split(">");
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Entry point
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	public static void generate(Path outDir) throws Exception
	{
		Bootstrap.init();
		for (Map.Entry<String, StatVectors.Spec> e : SPECS.entrySet())
		{
			final StatVectors.Built built = StatVectors.build(e.getValue());
			if (built == null)
			{
				throw new IllegalStateException("no template for " + e.getKey());
			}

			BUILT.put(e.getKey(), built);
		}

		for (Map.Entry<String, String[]> e : VARIANTS.entrySet())
		{
			final StatVectors.Built built = StatVectors.build(SPECS.get(e.getValue()[0]));
			for (int i = 1; i < e.getValue().length; i++)
			{
				applyMod(built.creature, e.getValue()[i]);
			}

			BUILT.put(e.getKey(), built);
		}

		writeMagicDam(outDir);
		writeMCrit(outDir);
		writeManaDam(outDir);
		writePhysDam(outDir);
		writeBlow(outDir);
		writeSkillCrit(outDir);
		writeBlowSuccess(outDir);
		writePhysEvasion(outDir);
		writeEffectSuccess(outDir);
		writeMagicSuccess(outDir);
		writeMagicAffected(outDir);
		writeMastery(outDir);
		writeTiming(outDir);
		writeResurrect(outDir);
		writeHeal(outDir);
		writeTraits(outDir);
		writeAttribute(outDir);
		// The skills and creatures files go last: they list what the cases above used.
		writeSkills(outDir);
		writeCreatures(outDir);
		System.out.println("skill-formulas: database access answered by the harness fake: " + DatabaseFactory.connections() + " fake connections, " + DatabaseFactory.statements().size() + " distinct statements");
	}

	// -- creatures + config + skills ------------------------------------------------------------------------------------------------------------------------

	/** Stats the skill formulas read, probed as {@code "STAT:init"}. */
	private static final String[] SKILL_PROBES =
	{
		"MCRITICAL_RATE:1",
		"MAGIC_CRIT_DMG:1",
		"PVP_MAGICAL_DMG:1",
		"PVP_MAGICAL_DEF:1",
		"PVE_MAGICAL_DMG:1",
		"PVP_PHYS_SKILL_DMG:1",
		"PVP_PHYS_SKILL_DEF:1",
		"PHYSICAL_SKILL_POWER:100",
		"PVE_PHYSICAL_DMG:1",
		"PVE_BOW_DMG:1",
		"PVE_BOW_SKILL_DMG:1",
		"CRITICAL_DAMAGE:1",
		"CRITICAL_DAMAGE_POS:1",
		"CRITICAL_DAMAGE_ADD:0",
		"DEFENCE_CRITICAL_DAMAGE:1",
		"DEFENCE_CRITICAL_DAMAGE_ADD:0",
		"DEFENCE_CRITICAL_RATE:100",
		"DEFENCE_CRITICAL_RATE_ADD:0",
		"DEBUFF_IMMUNITY:0",
		"DEBUFF_VULN:1",
		"BUFF_VULN:1",
		"BLOW_RATE:50",
		"SKILL_MASTERY:0",
		"SKILL_MASTERY_RATE:1",
		"MAGIC_SUCCESS_RES:1",
		"HEAL_EFFECT:100",
		"P_SKILL_EVASION:0",
		"MAGIC_REUSE_RATE:1",
		"P_REUSE:1",
		"DANCE_REUSE:1",
		"VENGEANCE_SKILL_MAGIC_DAMAGE:0",
		"VENGEANCE_SKILL_PHYSICAL_DAMAGE:0",
		"RANDOM_DAMAGE:0",
	};

	private static void writeCreatures(Path out) throws Exception
	{
		final Map<String, Object> root = Json.header("Creatures used by the skill formula vectors (built by StatVectors, like formulas_creatures.json) plus variants with extra stat functions, traits and invulnerabilities; config values the skill formulas read");
		root.put("note", "Each creature: base (the creature of formulas_creatures.json it is built from, or itself), mods (applied after the build; syntax: add:STAT:v = FuncAdd order 0x40, mul:STAT:v = FuncMul order 0x30, "
			+ "atk_trait:TRAIT:v / def_trait:TRAIT:v = the trait value set with count 1, invul:TRAIT = trait invulnerability count 1), input (what StatVectors builds), expected (user_info/raw as in the stats files), "
			+ "skill_probes (calcStat(STAT, init) for the stats the skill formulas read, key 'stat@init'), traits (attack/defence: TRAIT -> [value, count] for every trait that is not at its default 1.0 / count 0; invul: list), "
			+ "elements (attack_power / defence_value by element index 0 fire, 1 water, 2 wind, 3 earth, 4 holy, 5 dark, via getAttackElementValue / getDefenseElementValue; attack_element = getAttackElement()), flags.");
		final Map<String, Object> config = Json.obj();
		config.put("MagicFailures", PlayerConfig.ALT_GAME_MAGICFAILURES);
		config.put("RandomizeMagicalSkillDamage", PlayerConfig.RANDOMIZE_MAGICAL_SKILL_DAMAGE);
		config.put("RandomizePhysicalSkillDamage", PlayerConfig.RANDOMIZE_PHYSICAL_SKILL_DAMAGE);
		config.put("RandomizeAutoAttackDamage", PlayerConfig.RANDOMIZE_AUTO_ATTACK_DAMAGE);
		config.put("AltShieldBlocks", PlayerConfig.ALT_GAME_SHIELD_BLOCKS);
		config.put("AltPerfectShieldBlockRate", PlayerConfig.ALT_PERFECT_SHLD_BLOCK);
		config.put("CalculateMagicSuccessBySkillMagicLevel", PlayerConfig.CALCULATE_MAGIC_SUCCESS_BY_SKILL_MAGIC_LEVEL);
		config.put("MinAbnormalStateSuccessRate", PlayerConfig.MIN_ABNORMAL_STATE_SUCCESS_RATE);
		config.put("MaxAbnormalStateSuccessRate", PlayerConfig.MAX_ABNORMAL_STATE_SUCCESS_RATE);
		config.put("DebuffDurationUsesResists", GeneralConfig.DEBUFF_DURATION_USES_RESISTS);
		config.put("MinNPCLevelForDmgPenalty", NpcConfig.MIN_NPC_LEVEL_DMG_PENALTY);
		config.put("MinNPCLevelForMagicPenalty", NpcConfig.MIN_NPC_LEVEL_MAGIC_PENALTY);
		config.put("NpcDmgPenalty", Json.compact(floats(NpcConfig.NPC_DMG_PENALTY)));
		config.put("NpcCritDmgPenalty", Json.compact(floats(NpcConfig.NPC_CRIT_DMG_PENALTY)));
		config.put("NpcSkillDmgPenalty", Json.compact(floats(NpcConfig.NPC_SKILL_DMG_PENALTY)));
		config.put("NpcSkillChancePenalty", Json.compact(floats(NpcConfig.NPC_SKILL_CHANCE_PENALTY)));
		final List<Object> notOne = new ArrayList<>();
		for (Field f : ClassBalanceConfig.class.getDeclaredFields())
		{
			if (f.getType() == float[].class)
			{
				for (float v : (float[]) f.get(null))
				{
					if (v != 1.0f)
					{
						notOne.add(f.getName());
						break;
					}
				}
			}
		}

		config.put("ClassBalance_arrays_not_all_1", Json.compact(notOne));
		root.put("config", config);

		final List<Object> cases = new ArrayList<>();
		for (Map.Entry<String, StatVectors.Built> e : BUILT.entrySet())
		{
			final Creature c = e.getValue().creature;
			final Map<String, Object> m = Json.obj();
			m.put("name", e.getKey());
			final String[] v = VARIANTS.get(e.getKey());
			m.put("base", v == null ? e.getKey() : v[0]);
			final List<Object> mods = new ArrayList<>();
			if (v != null)
			{
				for (int i = 1; i < v.length; i++)
				{
					mods.add(v[i]);
				}
			}

			m.put("mods", Json.compact(mods));
			m.put("input", StatVectors.input(e.getValue()));
			m.put("expected", StatVectors.dump(c));

			final Map<String, Object> probes = Json.obj();
			for (String probe : SKILL_PROBES)
			{
				final String[] parts = probe.split(":");
				probes.put(parts[0].toLowerCase() + "@" + parts[1], c.calcStat(Stat.valueOf(parts[0]), Double.parseDouble(parts[1])));
			}

			m.put("skill_probes", Json.compact(probes));
			m.put("traits", traits(c));
			m.put("elements", elements(c));

			final Map<String, Object> flags = Json.obj();
			flags.put("is_player", c.isPlayer());
			flags.put("is_npc", c.isNpc());
			flags.put("is_raid", c.isRaid());
			flags.put("is_attackable", c.isAttackable());
			flags.put("is_mage_class", c.isPlayer() && c.asPlayer().isMageClass());
			flags.put("class_id", c.isPlayer() ? c.asPlayer().getPlayerClass().getId() : -1);
			flags.put("attack_type", c.getAttackType().name());
			flags.put("has_shield", c.getSecondaryWeaponItem() != null);
			flags.put("active_weapon_item_id", c.getActiveWeaponItem() == null ? 0 : c.getActiveWeaponItem().getId());
			flags.put("max_hp", c.getMaxHp());
			flags.put("max_recoverable_hp", c.getMaxRecoverableHp());
			m.put("flags", Json.compact(flags));
			cases.add(m);
		}

		root.put("creatures", cases);
		Json.write(out.resolve("skill_formulas_creatures.json"), root);
		System.out.println("skill-formulas: wrote skill_formulas_creatures.json (" + cases.size() + " creatures)");
	}

	private static Object traits(Creature c)
	{
		final Map<String, Object> attack = Json.obj();
		final Map<String, Object> defence = Json.obj();
		final List<Object> invul = new ArrayList<>();
		for (TraitType t : TraitType.values())
		{
			final int i = t.ordinal();
			if ((c.getStat().getAttackTraits()[i] != 1.0f) || (c.getStat().getAttackTraitsCount()[i] != 0))
			{
				attack.put(t.name(), Json.compact(Arrays.asList(c.getStat().getAttackTraits()[i], c.getStat().getAttackTraitsCount()[i])));
			}

			if ((c.getStat().getDefenceTraits()[i] != 1.0f) || (c.getStat().getDefenceTraitsCount()[i] != 0))
			{
				defence.put(t.name(), Json.compact(Arrays.asList(c.getStat().getDefenceTraits()[i], c.getStat().getDefenceTraitsCount()[i])));
			}

			if (c.getStat().getTraitsInvul()[i] != 0)
			{
				invul.add(t.name());
			}
		}

		final Map<String, Object> m = Json.obj();
		m.put("attack", Json.compact(attack));
		m.put("defence", Json.compact(defence));
		m.put("invul", Json.compact(invul));
		return Json.compact(m);
	}

	private static Object elements(Creature c)
	{
		final List<Object> attack = new ArrayList<>();
		final List<Object> defence = new ArrayList<>();
		for (byte e = 0; e < 6; e++)
		{
			attack.add(c.getAttackElementValue(e));
			defence.add(c.getDefenseElementValue(e));
		}

		final Map<String, Object> m = Json.obj();
		m.put("attack_power", Json.compact(attack));
		m.put("defence_value", Json.compact(defence));
		m.put("attack_element", (int) c.getAttackElement());
		m.put("default_defence_value", c.getDefenseElementValue((byte) -1));
		return Json.compact(m);
	}

	private static List<Object> floats(float[] values)
	{
		final List<Object> list = new ArrayList<>();
		for (float v : values)
		{
			list.add(v);
		}

		return list;
	}

	private static void writeSkills(Path out) throws Exception
	{
		final Map<String, Object> root = Json.header("Skills used by the skill formula vectors (Skill getters of the datapack skill, as the formulas read them)");
		root.put("note", "key = 'id:level'. power is Skill.getPower(), pvp_power / pve_power are getPower(true,false) / getPower(false,true). effects are the class names of the GENERAL effect scope in order.");
		final List<Object> skills = new ArrayList<>();
		for (String key : SKILLS_USED)
		{
			final Skill s = skill(key);
			final Map<String, Object> m = Json.obj();
			m.put("key", key);
			m.put("id", s.getId());
			m.put("level", s.getLevel());
			m.put("name", s.getName());
			m.put("is_magic", s.isMagic());
			m.put("is_physical", s.isPhysical());
			m.put("is_static", s.isStatic());
			m.put("is_active", s.isActive());
			m.put("is_passive", s.isPassive());
			m.put("is_toggle", s.isToggle());
			m.put("is_debuff", s.isDebuff());
			m.put("has_negative_effect", s.hasNegativeEffect());
			m.put("power", s.getPower());
			m.put("pvp_power", s.getPower(true, false));
			m.put("pve_power", s.getPower(false, true));
			m.put("magic_level", s.getMagicLevel());
			m.put("lvl_bonus_rate", s.getLvlBonusRate());
			m.put("activate_rate", s.getActivateRate());
			m.put("min_chance", s.getMinChance());
			m.put("max_chance", s.getMaxChance());
			m.put("basic_property", s.getBasicProperty().name());
			m.put("trait", s.getTraitType().name());
			m.put("element", (int) s.getElement());
			m.put("element_power", s.getElementPower());
			m.put("base_crit_rate", s.getBaseCritRate());
			m.put("blow_chance", s.getBlowChance());
			m.put("hit_time", s.getHitTime());
			m.put("cool_time", s.getCoolTime());
			m.put("reuse_delay", s.getReuseDelay());
			m.put("static_reuse", s.isStaticReuse());
			m.put("mp_consume", s.getMpConsume());
			m.put("item_consume_count", s.getItemConsumeCount());
			m.put("abnormal_time", s.getAbnormalTime());
			m.put("abnormal_instant", s.isAbnormalInstant());
			m.put("ignore_shield", s.ignoreShield());
			m.put("use_soul_shot", s.useSoulShot());
			m.put("use_spirit_shot", s.useSpiritShot());
			m.put("is_channeling", s.isChanneling());
			m.put("channeling_skill_id", s.getChannelingSkillId());
			m.put("cast_range", s.getCastRange());
			m.put("effects", Json.compact(effectNames(s)));
			skills.add(Json.compact(m));
		}

		root.put("skills", skills);
		Json.write(out.resolve("skill_formulas_skills.json"), root);
		System.out.println("skill-formulas: wrote skill_formulas_skills.json (" + skills.size() + " skills)");
	}

	// -- magic damage ---------------------------------------------------------------------------------------------------------------------------------------

	private static final String[] MAGIC_DAM_SKILLS =
	{
		"1177:5", // Wind Strike, wind 20
		"1230:28", // Prominence, fire 20
		"1148:13", // Death Spike, dark 20
		"4001:12", // NPC Windstrike (magic level 95)
		"49:26", // Divine Strike, holy 20
		"1263:13" // Curse Gloom: magic damage + debuff, dark 20
	};

	/** Attacker > target pairs for the magic skills (also used by the success files). */
	private static final String[] MAGIC_PAIRS =
	{
		"archmage76>npc_20677",
		"archmage76>npc_20239",
		"archmage76>npc_18366",
		"archmage76>npc_25010",
		"archmage76>duelist76",
		"archmage76>gladiator40",
		"archmage76_elem>npc_20677_res",
		"archmage76_elem>npc_21090",
		"archmage76_pvpmods>duelist76_magres",
		"archmage76_pvpmods>npc_20103",
		"cardinal76>npc_21090",
		"duelist76>npc_20677",
		"duelist76>archmage76",
		"npc_20677>duelist76",
		"npc_21090>archmage76",
		"npc_25010>duelist76",
		"npc_20103>warrior20",
		"warrior20>npc_20201",
		"newbie1>npc_20001"
	};

	private static void writeMagicDam(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcMagicDam(attacker, target, skill, shld, sps, bss, mcrit)",
			"result = damage (double, before the (int) cast of the MagicalDamage effect). shots: sps / bss are the booleans passed to the function. mcrit is the passed flag (calcMCrit is in skill_formulas_mcrit.json). "
				+ "With MagicFailures on, calcMagicSuccess draws first (Rnd.get(100) < rate = success); on failure a player attacker draws a second calcMagicSuccess (half damage when it succeeds and the level difference is <= 9, else damage 1), "
				+ "then the weapon random multiplier draws (RandomizeMagicalSkillDamage). shld 2 returns 1 before any draw. intermediate: the numbers the function reads from the pair. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		// {shots, mcrit, script index, shld}
		final Object[][] variants =
		{
			{
				"none",
				false,
				0,
				0
			},
			{
				"none",
				false,
				1,
				0
			},
			{
				"none",
				false,
				2,
				0
			},
			{
				"none",
				true,
				0,
				0
			},
			{
				"none",
				true,
				1,
				0
			},
			{
				"sps",
				false,
				0,
				0
			},
			{
				"sps",
				true,
				0,
				0
			},
			{
				"bss",
				false,
				0,
				0
			},
			{
				"bss",
				true,
				2,
				0
			},
			{
				"none",
				false,
				0,
				1
			},
			{
				"none",
				false,
				0,
				2
			},
			{
				"bss",
				false,
				3,
				0
			}
		};
		final double[][] scripts =
		{
			script(0.0, 0.5),
			script(0.99, 0.0, 0.5),
			script(0.99, 0.99, 0.5),
			seeded(0x5C111L, 4)
		};
		for (String pairName : MAGIC_PAIRS)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			place(a, t, "front", 0);
			for (String key : MAGIC_DAM_SKILLS)
			{
				final Skill skill = skill(key);
				final Map<String, Object> inter = pairInfo(a, t, skill);
				for (Object[] v : variants)
				{
					final boolean sps = v[0].equals("sps");
					final boolean bss = v[0].equals("bss");
					final boolean mcrit = (Boolean) v[1];
					final int r = (Integer) v[2];
					final byte shld = (byte) (int) (Integer) v[3];
					final Run run = run(scripts[r], () -> Formulas.calcMagicDam(a, t, skill, shld, sps, bss, mcrit));
					final Map<String, Object> c = pairCase("mdam_" + pairName + "_" + key + "_" + v[0] + (mcrit ? "_mcrit" : "") + "_s" + shld + "_r" + r, p[0], p[1]);
					c.put("skill", key);
					c.put("shld", (int) shld);
					c.put("sps", sps);
					c.put("bss", bss);
					c.put("mcrit", mcrit);
					c.put("intermediate", Json.compact(inter));
					putDraws(c, run);
					c.put("result", run.result);
					cases.add(Json.compact(c));
				}
			}
		}

		write(out, "magic_dam", root, cases, false);
	}

	// -- magic crit -----------------------------------------------------------------------------------------------------------------------------------------

	private static void writeMCrit(Path out) throws Exception
	{
		final Map<String, Object> root = header("Creature.getMCriticalHit(target, skill) and Formulas.calcMCrit(rate)",
			"kind 'rate': m_critical = attacker.getMCriticalHit(target, skill) (the 'rate' calcMCrit compares; no draw). kind 'calc': Formulas.calcMCrit(rate) = rate > Rnd.get(1000) for fixed rates and draws. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		for (String name : BUILT.keySet())
		{
			final Creature a = creature(name);
			for (String targetName : new String[]
			{
				"npc_20677",
				"duelist76"
			})
			{
				final Creature t = creature(targetName);
				for (String key : new String[]
				{
					"1177:5",
					"1011:18",
					"1102:6"
				})
				{
					final Skill skill = skill(key);
					final Map<String, Object> c = Json.obj();
					c.put("kind", "rate");
					c.put("id", "mcrit_rate_" + name + ">" + targetName + "_" + key);
					c.put("attacker", name);
					c.put("target", targetName);
					c.put("skill", key);
					c.put("result", a.getMCriticalHit(t, skill));
					cases.add(Json.compact(c));
				}
			}
		}

		final int[] rates =
		{
			-5,
			0,
			1,
			10,
			50,
			100,
			250,
			999,
			1000,
			2000
		};
		final int[] draws =
		{
			0,
			1,
			9,
			10,
			11,
			49,
			50,
			99,
			250,
			500,
			999
		};
		for (int rate : rates)
		{
			for (int d : draws)
			{
				final Run r = run(script(draw1000(d)), () -> Formulas.calcMCrit(rate));
				final Map<String, Object> c = Json.obj();
				c.put("kind", "calc");
				c.put("id", "mcrit_calc_r" + rate + "_d" + d);
				c.put("rate", rate);
				c.put("draw", d);
				putDraws(c, r);
				c.put("result", r.result);
				cases.add(Json.compact(c));
			}
		}

		write(out, "mcrit", root, cases, false);
	}

	// -- mana damage ----------------------------------------------------------------------------------------------------------------------------------------

	private static void writeManaDam(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcManaDam(attacker, target, skill, shld, sps, bss, mcrit)",
			"result = mana damage (double). Order of draws: calcMagicSuccess (MagicFailures; failure halves the damage for player attackers). shld 2 returns 1 before any draw. intermediate adds target_max_mp. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final String[] pairs =
		{
			"archmage76>npc_20677",
			"archmage76>npc_20239",
			"archmage76>npc_18366",
			"archmage76>npc_25010",
			"archmage76>duelist76",
			"archmage76_pvpmods>archmage76",
			"cardinal76>gladiator40",
			"duelist76>npc_21090",
			"npc_20677>archmage76",
			"npc_25010>duelist76"
		};
		final Object[][] variants =
		{
			{
				"none",
				false,
				0,
				0
			},
			{
				"none",
				false,
				1,
				0
			},
			{
				"none",
				true,
				0,
				0
			},
			{
				"sps",
				false,
				0,
				0
			},
			{
				"bss",
				true,
				0,
				0
			},
			{
				"bss",
				false,
				1,
				0
			},
			{
				"none",
				false,
				0,
				1
			},
			{
				"none",
				false,
				0,
				2
			}
		};
		final double[][] scripts =
		{
			script(0.0),
			script(0.99)
		};
		for (String pairName : pairs)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			place(a, t, "front", 0);
			for (String key : new String[]
			{
				"1102:6"
			})
			{
				final Skill skill = skill(key);
				final Map<String, Object> inter = pairInfo(a, t, skill);
				inter.put("target_max_mp", t.getMaxMp());
				for (Object[] v : variants)
				{
					final boolean sps = v[0].equals("sps");
					final boolean bss = v[0].equals("bss");
					final boolean mcrit = (Boolean) v[1];
					final int r = (Integer) v[2];
					final byte shld = (byte) (int) (Integer) v[3];
					final Run run = run(scripts[r], () -> Formulas.calcManaDam(a, t, skill, shld, sps, bss, mcrit));
					final Map<String, Object> c = pairCase("mana_" + pairName + "_" + key + "_" + v[0] + (mcrit ? "_mcrit" : "") + "_s" + shld + "_r" + r, p[0], p[1]);
					c.put("skill", key);
					c.put("shld", (int) shld);
					c.put("sps", sps);
					c.put("bss", bss);
					c.put("mcrit", mcrit);
					c.put("intermediate", Json.compact(inter));
					putDraws(c, run);
					c.put("result", run.result);
					cases.add(Json.compact(c));
				}
			}
		}

		write(out, "mana_dam", root, cases, false);
	}

	// -- physical skill damage -----------------------------------------------------------------------------------------------------------------------------

	/** {attacker > target, skill, skill, ...}. The first three rows are also placed at the side and the back. */
	private static final String[][] PHYS_ROWS =
	{
		{
			"duelist76>npc_20239",
			"3:9",
			"1:37",
			"223:49"
		},
		{
			"duelist76>npc_20677",
			"3:9",
			"1:37"
		},
		{
			"duelist76>duelist76_dbi",
			"3:9",
			"1:37",
			"223:49"
		},
		{
			"duelist76>npc_18366",
			"3:9"
		},
		{
			"duelist76>npc_25010",
			"3:9",
			"1:37"
		},
		{
			"duelist76>gladiator40",
			"3:9",
			"1:37"
		},
		{
			"duelist76_skillmods>gladiator40_defmods",
			"3:9",
			"1:37"
		},
		{
			"duelist76_skillmods>npc_21090",
			"3:9",
			"1:37"
		},
		{
			"duelist76_skillmods>npc_20677_res",
			"3:9",
			"1:37",
			"223:49"
		},
		{
			"gladiator40>duelist76",
			"3:9"
		},
		{
			"gladiator40>npc_20239",
			"3:9",
			"1:37"
		},
		{
			"warrior20>npc_20201",
			"3:9"
		},
		{
			"newbie1>npc_20001",
			"3:9"
		},
		{
			"adventurer76>npc_20677",
			"3:9",
			"1:37"
		},
		{
			"sagittarius76>npc_21090",
			"56:1",
			"101:1"
		},
		{
			"sagittarius76>npc_18366",
			"56:1"
		},
		{
			"sagittarius76>archmage76",
			"56:1",
			"101:1"
		},
		{
			"sagittarius76>npc_20677_res",
			"56:1"
		},
		{
			"npc_20677>duelist76",
			"4032:12"
		},
		{
			"npc_25010>duelist76",
			"4032:12"
		},
		{
			"npc_21090>archmage76",
			"4032:12"
		},
		{
			"npc_21001>sagittarius76",
			"4032:12"
		}
	};

	private static void writePhysDam(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcPhysDam(attacker, target, skill, shld, crit, ss) with a skill",
			"result = damage (double, before the (int) cast of PhysicalDamage). With RandomizePhysicalSkillDamage on the weapon random multiplier draws twice (once for the config, once unconditionally: Java quirk), "
				+ "the crit (calcCrit with the skill) is a separate call (skill_formulas_skill_crit.json) and passed as a flag. shld 2 returns 1 before any draw. intermediate adds the trait bonus (calcAttackTraitBonus) and attribute bonus the function multiplies in. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final double[][] scripts =
		{
			script(0.0, 0.0),
			script(0.5, 0.5),
			script(0.999, 0.3)
		};
		for (int row = 0; row < PHYS_ROWS.length; row++)
		{
			final String[] p = pair(PHYS_ROWS[row][0]);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			final String[] relations = row < 3 ? new String[]
			{
				"front",
				"side",
				"back"
			} : new String[]
			{
				"front"
			};
			for (int k = 1; k < PHYS_ROWS[row].length; k++)
			{
				final String key = PHYS_ROWS[row][k];
				final Skill skill = skill(key);
				for (String relation : relations)
				{
					final Map<String, Object> placement = place(a, t, relation, 0);
					final Map<String, Object> inter = pairInfo(a, t, skill);
					inter.put("attack_trait_bonus", Formulas.calcAttackTraitBonus(a, t));
					inter.put("attribute_bonus", Formulas.calcAttributeBonus(a, t, skill));
					inter.put("weapon_type", a.getAttackType().name());
					for (int shld = 0; shld <= 2; shld++)
					{
						for (int combo = 0; combo < 4; combo++)
						{
							final boolean crit = (combo & 1) != 0;
							final boolean ss = (combo & 2) != 0;
							if ((shld == 2) && (combo != 0))
							{
								continue;
							}

							for (int s = 0; s < scripts.length; s++)
							{
								if (((shld == 2) && (s != 0)) || ((shld == 1) && (s != 1)) || ((shld == 0) && (combo > 0) && (s == 2)))
								{
									continue;
								}

								final byte shield = (byte) shld;
								final Run run = run(scripts[s], () -> Formulas.calcPhysDam(a, t, skill, shield, crit, ss));
								final Map<String, Object> c = pairCase("pskill_" + PHYS_ROWS[row][0] + "_" + key + "_" + relation + "_s" + shld + (crit ? "_crit" : "_nocrit") + (ss ? "_ss" : "_noss") + "_r" + s, p[0], p[1]);
								c.put("skill", key);
								c.put("placement", Json.compact(placement));
								c.put("shld", shld);
								c.put("crit", crit);
								c.put("ss", ss);
								c.put("intermediate", Json.compact(inter));
								putDraws(c, run);
								c.put("result", run.result);
								cases.add(Json.compact(c));
							}
						}
					}
				}
			}
		}

		write(out, "phys_dam", root, cases, true);
	}

	// -- blow and backstab ----------------------------------------------------------------------------------------------------------------------------------

	private static final String[] BLOW_PAIRS =
	{
		"adventurer76>npc_20677",
		"adventurer76>npc_21090",
		"adventurer76>npc_18366",
		"adventurer76>npc_25010",
		"adventurer76>duelist76",
		"adventurer76>archmage76",
		"adventurer76_blow>npc_20677_res",
		"duelist76_skillmods>gladiator40_defmods",
		"npc_20677>duelist76"
	};

	private static void writeBlow(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcBlowDamage and Formulas.calcBackstabDamage (attacker, target, skill, shld, ss)",
			"kind 'blow' = calcBlowDamage, kind 'backstab' = calcBackstabDamage, both for every skill (they only read the skill's power, trait, element). result = damage (double, at least 1). shld 2 returns 1 before any draw. "
				+ "One weapon-random draw (RandomizePhysicalSkillDamage). The crit that FatalBlow / Backstab add around these (calcCrit with the skill, x2) is not part of the function. proximity bonus: behind 1.2, front 1, side 1.1. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final double[][] scripts =
		{
			script(0.0),
			script(0.9)
		};
		for (String pairName : BLOW_PAIRS)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			for (String key : new String[]
			{
				"16:24",
				"30:37",
				"344:1"
			})
			{
				if (key.equals("344:1") && !pairName.startsWith("adventurer76>npc_20677") && !pairName.startsWith("adventurer76>duelist76"))
				{
					continue;
				}

				final Skill skill = skill(key);
				for (String kind : new String[]
				{
					"blow",
					"backstab"
				})
				{
					for (String relation : new String[]
					{
						"front",
						"side",
						"back"
					})
					{
						final Map<String, Object> placement = place(a, t, relation, 0);
						final Map<String, Object> inter = pairInfo(a, t, skill);
						inter.put("attribute_bonus", Formulas.calcAttributeBonus(a, t, skill));
						inter.put("general_trait_bonus", Formulas.calcGeneralTraitBonus(a, t, skill.getTraitType(), false));
						inter.put("weapon_trait_bonus", Formulas.calcWeaponTraitBonus(a, t));
						for (int shld = 0; shld <= 2; shld++)
						{
							for (int combo = 0; combo < 2; combo++)
							{
								final boolean ss = combo == 1;
								if ((shld == 2) && (ss || !relation.equals("front")))
								{
									continue;
								}

								for (int s = 0; s < scripts.length; s++)
								{
									if (((shld == 2) && (s != 0)) || ((shld == 1) && ((s != 0) || ss)))
									{
										continue;
									}

									final byte shield = (byte) shld;
									final Run run = run(scripts[s], () -> kind.equals("blow") ? Formulas.calcBlowDamage(a, t, skill, shield, ss) : Formulas.calcBackstabDamage(a, t, skill, shield, ss));
									final Map<String, Object> c = pairCase("blow_" + kind + "_" + pairName + "_" + key + "_" + relation + "_s" + shld + (ss ? "_ss" : "_noss") + "_r" + s, p[0], p[1]);
									c.put("kind", kind);
									c.put("skill", key);
									c.put("placement", Json.compact(placement));
									c.put("shld", shld);
									c.put("ss", ss);
									c.put("intermediate", Json.compact(inter));
									putDraws(c, run);
									c.put("result", run.result);
									cases.add(Json.compact(c));
								}
							}
						}
					}
				}
			}
		}

		write(out, "blow", root, cases, true);
	}

	// -- crit with a skill ----------------------------------------------------------------------------------------------------------------------------------

	private static void writeSkillCrit(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcCrit(attacker, target, skill)",
			"result = true is a critical hit: the rate is skill.getBaseCritRate() * 10 * STR bonus of the attacker (class balance multipliers are 1), compared with the target's defence rate: "
				+ "(calcStat(DEFENCE_CRITICAL_RATE, rate) + calcStat(DEFENCE_CRITICAL_RATE_ADD, 0)) > Rnd.get(1000). Draws: 0, 1000-d around the boundary. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final String[] pairs =
		{
			"adventurer76>npc_20677",
			"adventurer76>npc_18366",
			"adventurer76>duelist76",
			"duelist76>npc_20239",
			"duelist76>gladiator40_defmods",
			"duelist76_skillmods>archmage76",
			"npc_20677>duelist76"
		};
		for (String pairName : pairs)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			for (String key : new String[]
			{
				"1:37",
				"30:37",
				"344:1"
			})
			{
				final Skill skill = skill(key);
				final double rate = skill.getBaseCritRate() * 10 * BaseStat.STR.calcBonus(a);
				final double limit = t.getStat().calcStat(Stat.DEFENCE_CRITICAL_RATE, rate, null, null) + t.getStat().calcStat(Stat.DEFENCE_CRITICAL_RATE_ADD, 0, null, null);
				final TreeSet<Integer> draws = new TreeSet<>(Arrays.asList(0, 1, 50, 100, 150, 250, 500, 999));
				for (int d : new int[]
				{
					(int) limit - 1,
					(int) limit,
					(int) limit + 1
				})
				{
					draws.add(Math.max(0, Math.min(999, d)));
				}

				for (int d : draws)
				{
					final Run r = run(script(draw1000(d)), () -> Formulas.calcCrit(a, t, skill));
					final Map<String, Object> c = pairCase("scrit_" + pairName + "_" + key + "_d" + d, p[0], p[1]);
					c.put("skill", key);
					final Map<String, Object> inter = Json.obj();
					inter.put("rate", rate);
					inter.put("limit", limit);
					inter.put("draw", d);
					c.put("intermediate", Json.compact(inter));
					putDraws(c, r);
					c.put("result", r.result);
					cases.add(Json.compact(c));
				}
			}
		}

		write(out, "skill_crit", root, cases, false);
	}

	// -- blow success and physical skill evasion ----------------------------------------------------------------------------------------------------------

	private static void writeBlowSuccess(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcBlowSuccess(attacker, target, skill)",
			"result = Rnd.get(100) < calcStat(BLOW_RATE, blowChance * DEX bonus * sideMod) with sideMod 1 front, 2 behind, 1.5 side. draw = the d the script makes Rnd.get(100) return; the draws are 0, 50, 99 and the ones around the threshold. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final String[] pairs =
		{
			"adventurer76>npc_20677",
			"adventurer76>npc_21090",
			"adventurer76>duelist76",
			"adventurer76_blow>npc_20677_res",
			"adventurer76_blow>archmage76",
			"duelist76>npc_20239",
			"npc_20677>duelist76"
		};
		for (String pairName : pairs)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			for (String key : new String[]
			{
				"16:24",
				"30:37",
				"344:1"
			})
			{
				final Skill skill = skill(key);
				for (String relation : new String[]
				{
					"front",
					"side",
					"back"
				})
				{
					final Map<String, Object> placement = place(a, t, relation, 0);
					final Map<String, Object> inter = Json.obj();
					inter.put("blow_chance", skill.getBlowChance());
					inter.put("dex_bonus", BaseStat.DEX.calcBonus(a));
					forHundredDraws(() -> Formulas.calcBlowSuccess(a, t, skill), (d, run) ->
					{
						final Map<String, Object> c = pairCase("blowok_" + pairName + "_" + key + "_" + relation + "_d" + d, p[0], p[1]);
						c.put("skill", key);
						c.put("placement", Json.compact(placement));
						c.put("draw", d);
						c.put("intermediate", Json.compact(inter));
						putDraws(c, run);
						c.put("result", run.result);
						cases.add(Json.compact(c));
					});
				}
			}
		}

		write(out, "blow_success", root, cases, true);
	}

	private static void writePhysEvasion(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcPhysicalSkillEvasion(creature, target, skill)",
			"result = true when the target evades: Rnd.get(100) < calcStat(P_SKILL_EVASION, 0). Magic skills and debuffs return false without a draw. intermediate.p_skill_evasion is the target's rate. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final String[] pairs =
		{
			"duelist76>gladiator40_defmods",
			"duelist76>npc_20677",
			"sagittarius76>gladiator40_defmods",
			"archmage76>gladiator40_defmods",
			"adventurer76>gladiator40_defmods"
		};
		for (String pairName : pairs)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			for (String key : new String[]
			{
				"3:9",
				"56:1",
				"223:49",
				"1177:5"
			})
			{
				final Skill skill = skill(key);
				final Map<String, Object> inter = Json.obj();
				inter.put("p_skill_evasion", t.calcStat(Stat.P_SKILL_EVASION, 0, null, skill));
				forHundredDraws(() -> Formulas.calcPhysicalSkillEvasion(a, t, skill), (d, run) ->
				{
					final Map<String, Object> c = pairCase("pevasion_" + pairName + "_" + key + "_d" + d, p[0], p[1]);
					c.put("skill", key);
					c.put("draw", d);
					c.put("intermediate", Json.compact(inter));
					putDraws(c, run);
					c.put("result", run.result);
					cases.add(Json.compact(c));
				});
			}
		}

		write(out, "phys_evasion", root, cases, false);
	}

	// -- effect success -------------------------------------------------------------------------------------------------------------------------------------

	private static final String[] EFFECT_SKILLS =
	{
		"1069:42", // Sleep
		"1064:14", // Silence
		"1095:5", // Venom (CON)
		"1169:14", // Curse Fear
		"1263:13", // Curse Gloom (with damage)
		"1337:1", // Curse of Abyss (DEATH trait)
		"1108:1", // Seal of Flame: magic level 0
		"3040:1", // Special Ability: Critical Poison: magic level 0, POISON trait
		"4046:12", // NPC Sleep
		"223:49", // Sting: physical debuff, BLEED
		"101:1", // Stun Shot: physical debuff, SHOCK
		"1068:3", // Might: buff, activateRate -1
		"1077:1", // Focus: buff, magic level 0
		"4:2", // Dash: physical buff
		"1217:33" // Greater Heal: activateRate 0, no basic property
	};

	private static void writeEffectSuccess(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcEffectSuccess(attacker, target, skill)",
			"result = the effect lands. Debuff immunity (DEBUFF_IMMUNITY stat) returns false without a draw; activateRate -1 / > 99 / no basic property return true without a draw. "
				+ "Otherwise rate = clamp(baseMod * elementMod * traitMod * mAtkMod * buffDebuffMod, minChance, maxChance) (0 when traitMod is 0) and the effect lands when Rnd.get(100) < rate; "
				+ "'draw' is the d the script makes Rnd.get(100) return (0, 50, 99 and around the threshold; -1 = the call draws nothing). shots: the attacker's charged shots when the case ran (blessed spiritshot raises mAtkMod for magic skills). intermediate: the numbers of the pair. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final String[] pairs =
		{
			"archmage76>npc_20677",
			"archmage76>npc_20239",
			"archmage76>npc_18366",
			"archmage76>npc_25010",
			"archmage76>duelist76",
			"archmage76>npc_20677_res",
			"archmage76>npc_21090_invul",
			"archmage76>duelist76_dbi",
			"cardinal76>gladiator40",
			"duelist76>archmage76",
			"adventurer76>npc_20677",
			"sagittarius76>npc_21090",
			"npc_20677>duelist76",
			"npc_21090>archmage76"
		};
		for (String pairName : pairs)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			for (String key : EFFECT_SKILLS)
			{
				final Skill skill = skill(key);
				for (String shotKind : new String[]
				{
					"none",
					"bss"
				})
				{
					if (shotKind.equals("bss") && !skill.isMagic())
					{
						continue;
					}

					shots(a, shotKind);
					final Map<String, Object> state = shotState(a);
					if (shotKind.equals("bss") && !Boolean.TRUE.equals(state.get("bss")))
					{
						// The attacker cannot hold charged shots (no weapon): the case would repeat 'none'.
						shots(a, "none");
						continue;
					}

					final Map<String, Object> inter = Json.obj();
					inter.put("attacker_m_atk", a.getMAtk(null, null));
					inter.put("target_m_def", t.getMDef(null, null));
					inter.put("target_level", (int) t.getLevel());
					inter.put("target_str", t.getSTR());
					inter.put("target_dex", t.getDEX());
					inter.put("target_con", t.getCON());
					inter.put("target_int", t.getINT());
					inter.put("target_men", t.getMEN());
					inter.put("target_wit", t.getWIT());
					inter.put("element_bonus", Formulas.calcAttributeBonus(a, t, skill));
					inter.put("trait_bonus", Formulas.calcGeneralTraitBonus(a, t, skill.getTraitType(), false));
					inter.put("debuff_immunity", t.calcStat(Stat.DEBUFF_IMMUNITY, 0, a, skill));
					forHundredDraws(() -> Formulas.calcEffectSuccess(a, t, skill), (d, run) ->
					{
						final Map<String, Object> c = pairCase("effok_" + pairName + "_" + key + "_" + shotKind + "_d" + d, p[0], p[1]);
						c.put("skill", key);
						c.put("shots", Json.compact(state));
						c.put("draw", d);
						c.put("intermediate", Json.compact(inter));
						putDraws(c, run);
						c.put("result", run.result);
						cases.add(Json.compact(c));
					});
					shots(a, "none");
				}
			}
		}

		write(out, "effect_success", root, cases, false);
	}

	// -- magic success --------------------------------------------------------------------------------------------------------------------------------------

	private static void writeMagicSuccess(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcMagicSuccess(attacker, target, skill)",
			"result = Rnd.get(100) < 100 - round(1.3^(targetLevel - levelRef) * npcPenalty * resistMod): levelRef is the skill's magic level when it is above 0 (CalculateMagicSuccessBySkillMagicLevel) else the attacker's level. "
				+ "power -1 or activateRate > 99 return true without a draw. 'draw' as in effect_success. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		for (String pairName : MAGIC_PAIRS)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			for (String key : new String[]
			{
				"1177:5",
				"1230:28",
				"1263:13",
				"1069:42",
				"1108:1",
				"1337:1"
			})
			{
				final Skill skill = skill(key);
				final Map<String, Object> inter = Json.obj();
				inter.put("attacker_level", (int) a.getLevel());
				inter.put("target_level", (int) t.getLevel());
				inter.put("target_is_attackable", t.isAttackable());
				inter.put("target_is_raid", t.isRaid());
				inter.put("magic_success_res", t.calcStat(Stat.MAGIC_SUCCESS_RES, 1, null, skill));
				forHundredDraws(() -> Formulas.calcMagicSuccess(a, t, skill), (d, run) ->
				{
					final Map<String, Object> c = pairCase("msucc_" + pairName + "_" + key + "_d" + d, p[0], p[1]);
					c.put("skill", key);
					c.put("draw", d);
					c.put("intermediate", Json.compact(inter));
					putDraws(c, run);
					c.put("result", run.result);
					cases.add(Json.compact(c));
				});
			}
		}

		write(out, "magic_success", root, cases, false);
	}

	// -- magic affected -------------------------------------------------------------------------------------------------------------------------------------

	private static void writeMagicAffected(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcMagicAffected(actor, target, skill)",
			"result = (attack - defence) / (attack + defence) + 0.5 * gaussian > 0, with attack = 2 * mAtk * traitBonus and defence = mDef for active skills with a negative effect (0 otherwise). "
				+ "The gaussian is Rnd.nextGaussian(): the harness Rnd gives it as 2u-1 for a script value u, so a replay in Rust must feed the recorded 'returned' value to its own nextGaussian. "
				+ "Debuffs against DEBUFF_IMMUNITY return false before the draw. The draws are fixed u values and the u that sits on the threshold (found by bisection on the real function). " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final String[] pairs =
		{
			"archmage76>npc_20677",
			"archmage76>npc_25010",
			"archmage76>duelist76",
			"archmage76>duelist76_dbi",
			"archmage76>npc_21090_invul",
			"archmage76>npc_20677_res",
			"duelist76>archmage76",
			"npc_20677>duelist76"
		};
		for (String pairName : pairs)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			for (String key : new String[]
			{
				"1102:6",
				"1069:42",
				"1169:14",
				"1177:5",
				"1011:18",
				"3040:1"
			})
			{
				final Skill skill = skill(key);
				final Map<String, Object> inter = Json.obj();
				inter.put("is_active", skill.isActive());
				inter.put("has_negative_effect", skill.hasNegativeEffect());
				inter.put("is_debuff", skill.isDebuff());
				inter.put("m_atk", a.getMAtk(t, skill));
				inter.put("m_def", t.getMDef(a, skill));
				inter.put("trait_bonus", Formulas.calcGeneralTraitBonus(a, t, skill.getTraitType(), false));
				// Bisection for the u where the result flips (the result grows with u).
				double lo = 0.0;
				double hi = 0.999999;
				final Supplier<Object> call = () -> Formulas.calcMagicAffected(a, t, skill);
				final boolean atLo = Boolean.TRUE.equals(run(script(lo), call).result);
				final boolean atHi = Boolean.TRUE.equals(run(script(hi), call).result);
				final TreeSet<Double> draws = new TreeSet<>(Arrays.asList(0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 0.999));
				if (!atLo && atHi)
				{
					for (int i = 0; i < 60; i++)
					{
						final double mid = (lo + hi) / 2;
						if (Boolean.TRUE.equals(run(script(mid), call).result))
						{
							hi = mid;
						}
						else
						{
							lo = mid;
						}
					}

					draws.add(lo);
					draws.add(hi);
				}

				for (double u : draws)
				{
					final Run run = run(script(u), call);
					final Map<String, Object> c = pairCase("maffected_" + pairName + "_" + key + "_u" + u, p[0], p[1]);
					c.put("skill", key);
					c.put("intermediate", Json.compact(inter));
					putDraws(c, run);
					c.put("result", run.result);
					cases.add(Json.compact(c));
				}
			}
		}

		write(out, "magic_affected", root, cases, false);
	}

	// -- mastery and abnormal time --------------------------------------------------------------------------------------------------------------------------

	private static void writeMastery(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcSkillMastery(actor, skill) and Formulas.calcEffectAbnormalTime(caster, target, skill)",
			"kind 'mastery': result = the skill was mastered: only players, not static skills, SKILL_MASTERY stat 1 (STR bonus) or 4 (INT bonus), x SKILL_MASTERY_RATE; Rnd.get(100) < chance (class balance multiplier 1). 'draw' as in effect_success. "
				+ "kind 'abnormal_time': result = the effect time in seconds (int): skill time (-1 for passive / toggle), x2 when mastery succeeds (one draw for players that have the stat), debuffs scaled by resistances (DebuffDurationUsesResists). "
				+ "Servitor targets (halved herb buff time) are not covered. The abnormal_time cases use script variants 'm0' (draw 0.0: mastery succeeds when it can) and 'm1' (0.99). " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		for (String name : new String[]
		{
			"duelist76_mastery1",
			"archmage76_mastery4",
			"cardinal76_mastery4",
			"duelist76",
			"npc_20677"
		})
		{
			final Creature a = creature(name);
			for (String key : new String[]
			{
				"3:9",
				"1011:18",
				"1069:42",
				"2001:1"
			})
			{
				final Skill skill = skill(key);
				final Map<String, Object> inter = Json.obj();
				inter.put("skill_mastery", a.getStat().calcStat(Stat.SKILL_MASTERY, 0, null, null));
				inter.put("skill_mastery_rate", a.getStat().calcStat(Stat.SKILL_MASTERY_RATE, 1, null, null));
				inter.put("str_bonus", BaseStat.STR.calcBonus(a));
				inter.put("int_bonus", BaseStat.INT.calcBonus(a));
				inter.put("is_static", skill.isStatic());
				forHundredDraws(() -> Formulas.calcSkillMastery(a, skill), (d, run) ->
				{
					final Map<String, Object> c = Json.obj();
					c.put("kind", "mastery");
					c.put("id", "mastery_" + name + "_" + key + "_d" + d);
					c.put("actor", name);
					c.put("skill", key);
					c.put("draw", d);
					c.put("intermediate", Json.compact(inter));
					putDraws(c, run);
					c.put("result", run.result);
					cases.add(Json.compact(c));
				});
			}
		}

		final String[] pairs =
		{
			"archmage76>duelist76",
			"archmage76_mastery4>npc_20677",
			"duelist76_mastery1>npc_21090",
			"cardinal76_mastery4>archmage76",
			"npc_20677>archmage76",
			"archmage76>npc_20677_res",
			"archmage76>npc_21090_invul",
			"adventurer76>cardinal76",
			"duelist76>gladiator40_defmods"
		};
		final String[] skills =
		{
			"1068:3",
			"1077:1",
			"1189:3",
			"4:2",
			"1217:33",
			"2001:1",
			"1069:42",
			"1064:14",
			"1095:5",
			"1263:13",
			"1108:1",
			"3040:1",
			"1337:1"
		};
		for (String pairName : pairs)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			for (String key : skills)
			{
				final Skill skill = skill(key);
				for (int m = 0; m < 2; m++)
				{
					final Run run = run(script(m == 0 ? 0.0 : 0.99), () -> Formulas.calcEffectAbnormalTime(a, t, skill));
					final Map<String, Object> c = pairCase("abntime_" + pairName + "_" + key + "_m" + m, p[0], p[1]);
					c.put("kind", "abnormal_time");
					c.put("skill", key);
					final Map<String, Object> inter = Json.obj();
					inter.put("skill_abnormal_time", skill.getAbnormalTime());
					inter.put("stat_mod_bonus", skill.getBasicProperty() == BaseStat.NONE ? null : skill.getBasicProperty().calcBonus(t));
					inter.put("trait_bonus", Formulas.calcGeneralTraitBonus(a, t, skill.getTraitType(), false));
					inter.put("lvl_bonus_mod", Formulas.calcLvlBonusMod(a, t, skill));
					inter.put("attribute_bonus", Formulas.calcAttributeBonus(a, t, skill));
					c.put("intermediate", Json.compact(inter));
					putDraws(c, run);
					c.put("result", run.result);
					cases.add(Json.compact(c));
				}
			}
		}

		write(out, "mastery", root, cases, false);
	}

	// -- cast and reuse time --------------------------------------------------------------------------------------------------------------------------------

	private static void writeTiming(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcAtkSpd(attacker, skill, skillTime) and the cast / reuse numbers of Creature.beginCast",
			"calc_atk_spd is the real Formulas.calcAtkSpd(creature, skill, hit_time + cool_time). cast_time and reuse_delay REPLAY the pure-number lines of Creature.beginCast (the method itself needs a world): "
				+ "skillTime = hit + cool; non-static skills go through calcAtkSpd; magic skills with charged sps / bss x0.6 (int); then the 550 ms (magic, base > 550) / 500 ms (non-static, base >= 500) floors; "
				+ "reuse_delay = (int)(reuse * MAGIC_REUSE_RATE | P_REUSE | DANCE_REUSE) unless static reuse / static skill, then (int)(x * SKILL_REUSE_MULTIPLIERS[class]) for players (1.0). The stat calls and calcAtkSpd are the real code. "
				+ "shots = the creature's charged shots when the case ran (a creature without a weapon cannot hold any). reuse_with_mastery is the reuse a mastered cast gets (100) when reuse_delay > 10.");
		final List<Object> cases = new ArrayList<>();
		final String[] skills =
		{
			"1177:5",
			"1230:28",
			"1011:18",
			"1015:15",
			"1068:3",
			"1069:42",
			"3:9",
			"16:24",
			"30:37",
			"56:1",
			"4:2",
			"4001:12",
			"4032:12",
			"2001:1"
		};
		for (String name : new String[]
		{
			"duelist76",
			"archmage76",
			"cardinal76",
			"sagittarius76",
			"adventurer76",
			"newbie1",
			"npc_20677",
			"npc_25010"
		})
		{
			final Creature c = creature(name);
			for (String key : skills)
			{
				final Skill skill = skill(key);
				for (String shotKind : new String[]
				{
					"none",
					"sps",
					"bss"
				})
				{
					if (!shotKind.equals("none") && !skill.isMagic())
					{
						continue;
					}

					shots(c, shotKind);
					final Map<String, Object> state = shotState(c);
					if (!shotKind.equals("none") && !Boolean.TRUE.equals(state.get(shotKind)))
					{
						shots(c, "none");
						continue;
					}

					int base = skill.getHitTime() + skill.getCoolTime();
					int skillTime = base;
					final int calc = Formulas.calcAtkSpd(c, skill, base);
					if (!skill.isChanneling() || (skill.getChannelingSkillId() == 0))
					{
						if (!skill.isStatic())
						{
							skillTime = calc;
						}

						if (skill.isMagic() && (c.isChargedShot(ShotType.SPIRITSHOTS) || c.isChargedShot(ShotType.BLESSED_SPIRITSHOTS)))
						{
							skillTime = (int) (0.6 * skillTime);
						}
					}

					if (skill.isMagic() && (base > 550) && (skillTime < 550))
					{
						skillTime = 550;
					}
					else if (!skill.isStatic() && (base >= 500) && (skillTime < 500))
					{
						skillTime = 500;
					}

					int reuseDelay;
					if (skill.isStaticReuse() || skill.isStatic())
					{
						reuseDelay = skill.getReuseDelay();
					}
					else if (skill.isMagic())
					{
						reuseDelay = (int) (skill.getReuseDelay() * c.calcStat(Stat.MAGIC_REUSE_RATE, 1, null, null));
					}
					else if (skill.isPhysical())
					{
						reuseDelay = (int) (skill.getReuseDelay() * c.calcStat(Stat.P_REUSE, 1, null, null));
					}
					else
					{
						reuseDelay = (int) (skill.getReuseDelay() * c.calcStat(Stat.DANCE_REUSE, 1, null, null));
					}

					if (c.isPlayable())
					{
						reuseDelay = (int) (reuseDelay * ClassBalanceConfig.SKILL_REUSE_MULTIPLIERS[c.asPlayer().getPlayerClass().getId()]);
					}

					final Map<String, Object> m = Json.obj();
					m.put("id", "timing_" + name + "_" + key + "_" + shotKind);
					m.put("creature", name);
					m.put("skill", key);
					m.put("shots", Json.compact(state));
					m.put("m_atk_spd", c.getMAtkSpd());
					m.put("p_atk_spd", c.getPAtkSpd());
					m.put("hit_plus_cool", base);
					m.put("calc_atk_spd", calc);
					m.put("cast_time", skillTime);
					m.put("reuse_delay", reuseDelay);
					m.put("reuse_with_mastery", reuseDelay > 10 ? 100 : reuseDelay);
					m.put("magic_reuse_rate", c.calcStat(Stat.MAGIC_REUSE_RATE, 1, null, null));
					m.put("p_reuse", c.calcStat(Stat.P_REUSE, 1, null, null));
					m.put("dance_reuse", c.calcStat(Stat.DANCE_REUSE, 1, null, null));
					cases.add(Json.compact(m));
					shots(c, "none");
				}
			}
		}

		write(out, "skill_timing", root, cases, false);
	}

	// -- resurrection ---------------------------------------------------------------------------------------------------------------------------------------

	private static void writeResurrect(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calculateSkillResurrectRestorePercent(baseRestorePercent, caster)",
			"result = the restore percent: 0 and 100 are returned unchanged; otherwise base * WIT bonus (+20 when the gain is over 20), at least base, at most 90. No draws. wit_bonus is BaseStat.WIT.calcBonus(caster).");
		final List<Object> cases = new ArrayList<>();
		for (String name : BUILT.keySet())
		{
			final Creature c = creature(name);
			for (double base : new double[]
			{
				0,
				1,
				5,
				10,
				20,
				30,
				40,
				50,
				60,
				70,
				80,
				90,
				95,
				100
			})
			{
				final Map<String, Object> m = Json.obj();
				m.put("id", "resurrect_" + name + "_" + (int) base);
				m.put("caster", name);
				m.put("base", base);
				m.put("wit_bonus", BaseStat.WIT.calcBonus(c));
				m.put("result", Formulas.calculateSkillResurrectRestorePercent(base, c));
				cases.add(Json.compact(m));
			}
		}

		write(out, "resurrect", root, cases, false);
	}

	// -- heal -----------------------------------------------------------------------------------------------------------------------------------------------

	private static void writeHeal(Path out) throws Exception
	{
		final Map<String, Object> root = header("Heal.onStart(effector, effected, skill) (handlers/skill/effects/Heal.java, the real effect object of the skill) and Formulas.calcMCrit",
			"The effect runs for real on the creatures: effected.setCurrentHp(hp_before), onStart, hp_after - hp_before is the healed amount (result). The number part is power + static shot bonus + sqrt(mAtkMul * mAtk), x HEAL_EFFECT stat, x class balance (1), "
				+ "x3 on a magic crit (calcMCrit(effector.getMCriticalHit) draws Rnd.get(1000) for magic, non-static skills), capped by max recoverable HP - current HP. "
				+ "shots = the effector's charged shots when the case ran (spiritshots need a weapon). effect_power is the 'power' param of the skill's Heal effect (read by reflection). Static skills (potions) skip the whole stat part: amount = power, no draw. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final Field powerField = Class.forName("handlers.skill.effects.Heal").getDeclaredField("_power");
		powerField.setAccessible(true);
		final String[] pairs =
		{
			"archmage76>duelist76",
			"archmage76>archmage76",
			"cardinal76>gladiator40",
			"cardinal76>cardinal76",
			"duelist76>archmage76",
			"newbie1>warrior20",
			"npc_20677>npc_21090"
		};
		final String[] skills =
		{
			"1011:18",
			"1217:33",
			"1015:15",
			"1218:33",
			"1011:1",
			"2038:1", // Quick Healing Potion: static skill (no stat, shot or crit part)
			"2287:6", // Elixir of Life
			"5146:10" // static "Heal"
		};
		final double[][] scripts =
		{
			script(0.0),
			script(0.999)
		};
		for (String pairName : pairs)
		{
			final String[] p = pair(pairName);
			final Creature effector = creature(p[0]);
			final Creature effected = creature(p[1]);
			for (String key : skills)
			{
				final Skill skill = skill(key);
				AbstractEffect heal = null;
				for (AbstractEffect e : skill.getEffects(EffectScope.GENERAL))
				{
					if (e.getClass().getSimpleName().equals("Heal"))
					{
						heal = e;
					}
				}

				if (heal == null)
				{
					throw new IllegalStateException("no Heal effect on " + key);
				}

				final AbstractEffect effect = heal;
				for (String shotKind : new String[]
				{
					"none",
					"sps",
					"bss"
				})
				{
					if (skill.isStatic() && !shotKind.equals("none"))
					{
						continue;
					}

					shots(effector, shotKind);
					final Map<String, Object> state = shotState(effector);
					if (!shotKind.equals("none") && !Boolean.TRUE.equals(state.get(shotKind)))
					{
						shots(effector, "none");
						continue;
					}

					for (int hpCase = 0; hpCase < 2; hpCase++)
					{
						for (int s = 0; s < scripts.length; s++)
						{
							final int maxRecoverable = effected.getMaxRecoverableHp();
							final double hpBefore = hpCase == 0 ? 1 : Math.max(1, maxRecoverable - 40);
							effected.setCurrentHp(hpBefore);
							final double before = effected.getCurrentHp();
							final Run run = run(scripts[s], () ->
							{
								effect.onStart(effector, effected, skill);
								return effected.getCurrentHp() - before;
							});
							final Map<String, Object> c = pairCase("heal_" + pairName + "_" + key + "_" + shotKind + "_hp" + hpCase + "_r" + s, p[0], p[1]);
							c.put("skill", key);
							c.put("shots", Json.compact(state));
							final Map<String, Object> inter = Json.obj();
							inter.put("effect_power", powerField.getDouble(effect));
							inter.put("hp_before", before);
							inter.put("max_hp", effected.getMaxHp());
							inter.put("max_recoverable_hp", maxRecoverable);
							inter.put("effector_m_atk", effector.getMAtk(effector, null));
							inter.put("effector_m_critical", effector.getMCriticalHit(effected, skill));
							inter.put("effector_is_mage_class", effector.isPlayer() && effector.asPlayer().isMageClass());
							inter.put("effector_weapon_crystal", effector.getActiveWeaponInstance() == null ? null : effector.getActiveWeaponInstance().getTemplate().getCrystalType().name());
							inter.put("heal_effect_probe", effected.calcStat(Stat.HEAL_EFFECT, 100, null, null));
							inter.put("skill_mp_consume", skill.getMpConsume());
							c.put("intermediate", Json.compact(inter));
							putDraws(c, run);
							c.put("result", run.result);
							cases.add(Json.compact(c));
							effected.setCurrentHp(effected.getMaxHp());
						}
					}

					shots(effector, "none");
				}
			}
		}

		write(out, "heal", root, cases, false);
	}

	// -- traits ---------------------------------------------------------------------------------------------------------------------------------------------

	private static void writeTraits(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcGeneralTraitBonus(attacker, target, traitType, ignoreResistance), calcWeaponTraitBonus, calcAttackTraitBonus",
			"kind 'general': every TraitType (name, ordinal, type 0 none / 1 weapon / 2 weakness / 3 resistance) x ignoreResistance. NONE returns 1, invulnerable targets 0, weakness traits need an attack AND a defence trait on the pair, "
				+ "resistance traits clamp(attackTrait - defenceTrait + 1, 0.05, 2). kind 'weapon': calcWeaponTraitBonus (1 - (defence trait of the attacker's weapon type - 1)); kind 'attack': calcAttackTraitBonus. No draws. Trait tables are in the creatures file.");
		final List<Object> cases = new ArrayList<>();
		final String[] pairs =
		{
			"duelist76_skillmods>npc_20677_res",
			"duelist76>npc_20677_res",
			"duelist76_skillmods>npc_21090_invul",
			"duelist76_skillmods>gladiator40_defmods",
			"duelist76>npc_21090_invul",
			"adventurer76>npc_20677_res",
			"sagittarius76>npc_20677_res"
		};
		for (String pairName : pairs)
		{
			final String[] p = pair(pairName);
			final Creature a = creature(p[0]);
			final Creature t = creature(p[1]);
			for (TraitType type : TraitType.values())
			{
				for (int ignore = 0; ignore < 2; ignore++)
				{
					final Map<String, Object> m = Json.obj();
					m.put("kind", "general");
					m.put("id", "trait_general_" + pairName + "_" + type.name() + "_i" + ignore);
					m.put("attacker", p[0]);
					m.put("target", p[1]);
					m.put("trait", type.name());
					m.put("ordinal", type.ordinal());
					m.put("trait_type", type.getType());
					m.put("ignore_resistance", ignore == 1);
					m.put("result", Formulas.calcGeneralTraitBonus(a, t, type, ignore == 1));
					cases.add(Json.compact(m));
				}
			}

			final Map<String, Object> w = Json.obj();
			w.put("kind", "weapon");
			w.put("id", "trait_weapon_" + pairName);
			w.put("attacker", p[0]);
			w.put("target", p[1]);
			w.put("attack_type", a.getAttackType().name());
			w.put("attack_type_trait", a.getAttackType().getTraitType().name());
			w.put("result", Formulas.calcWeaponTraitBonus(a, t));
			cases.add(Json.compact(w));
			final Map<String, Object> m = Json.obj();
			m.put("kind", "attack");
			m.put("id", "trait_attack_" + pairName);
			m.put("attacker", p[0]);
			m.put("target", p[1]);
			m.put("result", Formulas.calcAttackTraitBonus(a, t));
			cases.add(Json.compact(m));
		}

		write(out, "traits", root, cases, false);
	}

	// -- attribute ------------------------------------------------------------------------------------------------------------------------------------------

	private static void writeAttribute(Path out) throws Exception
	{
		final Map<String, Object> root = header("Formulas.calcAttributeBonus(attacker, target, skill)",
			"With a skill element: attack = attacker.getAttackElementValue(element) + skill.getElementPower(), defence = target.getDefenseElementValue(element). Without (skill null or element -1): the attacker's strongest element (getAttackElement) "
				+ "against the target's defence for it. diff > 0: min(1.025 + sqrt(diff^3 / 2) * 0.0001, 1.25); diff < 0: max(0.975 - sqrt(|diff|^3 / 2) * 0.0001, 0.75); 0: 1. No draws. skill 'null' = no skill (normal attack).");
		final List<Object> cases = new ArrayList<>();
		final String[] attackers =
		{
			"archmage76",
			"archmage76_elem",
			"duelist76",
			"duelist76_skillmods"
		};
		final String[] targets =
		{
			"npc_20677",
			"npc_20677_res",
			"npc_21090",
			"gladiator40_elemres",
			"archmage76_elem"
		};
		final String[] skills =
		{
			"1177:5", // wind
			"1230:28", // fire
			"1235:28", // water
			"3169:10", // earth
			"49:26", // holy
			"1148:13", // dark
			"731:1", // dark, power 55?
			"874:8", // fire 55
			"3:9", // no element
			"1:37", // no element
			"null"
		};
		for (String attackerName : attackers)
		{
			final Creature a = creature(attackerName);
			for (String targetName : targets)
			{
				if (attackerName.equals(targetName))
				{
					continue;
				}

				final Creature t = creature(targetName);
				for (String key : skills)
				{
					final Skill skill = key.equals("null") ? null : skill(key);
					final Map<String, Object> m = Json.obj();
					m.put("id", "attr_" + attackerName + ">" + targetName + "_" + key);
					m.put("attacker", attackerName);
					m.put("target", targetName);
					m.put("skill", key.equals("null") ? null : key);
					final int element = skill == null ? -1 : skill.getElement();
					final byte used = element != -1 ? (byte) element : a.getAttackElement();
					m.put("element_used", (int) used);
					m.put("attack_value", element != -1 ? a.getAttackElementValue(used) + skill.getElementPower() : a.getAttackElementValue(used));
					m.put("defence_value", t.getDefenseElementValue(used));
					m.put("result", Formulas.calcAttributeBonus(a, t, skill));
					cases.add(Json.compact(m));
				}
			}
		}

		write(out, "attribute", root, cases, false);
	}
}
