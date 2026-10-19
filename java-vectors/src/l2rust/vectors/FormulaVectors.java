package l2rust.vectors;

import java.lang.reflect.Field;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.function.Supplier;

import org.l2jmobius.commons.database.DatabaseFactory;
import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.config.NpcConfig;
import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.config.custom.ClassBalanceConfig;
import org.l2jmobius.gameserver.data.xml.HitConditionBonusData;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.entity.Location;
import org.l2jmobius.gameserver.entity.actor.Creature;
import org.l2jmobius.gameserver.entity.actor.Player;
import org.l2jmobius.gameserver.entity.item.Weapon;
import org.l2jmobius.gameserver.mechanics.skill.Skill;
import org.l2jmobius.gameserver.mechanics.stats.Formulas;
import org.l2jmobius.gameserver.mechanics.stats.Stat;
import org.l2jmobius.gameserver.taskmanagers.GameTimeTaskManager;

/**
 * Formula vectors (M4-JV-01): the real {@code Formulas} methods of L2JMobius on creatures built by {@link StatVectors}, with every {@code Rnd} draw scripted and recorded.
 * <p>
 * Files (all in the output directory):
 * <ul>
 * <li>{@code formulas_creatures.json}: the creatures (input + expected stats from {@link StatVectors}), the config values the formulas read.</li>
 * <li>{@code formulas_hit_miss.json}: {@code calcHitMiss}</li>
 * <li>{@code formulas_crit.json}: {@code calcCrit} (normal attack)</li>
 * <li>{@code formulas_shield.json}: {@code calcShldUse}</li>
 * <li>{@code formulas_phys_dam.json}: {@code calcPhysDam} (normal attack, shield result x crit x soulshot)</li>
 * <li>{@code formulas_attack_hit.json}: the sequence of {@code Creature.doAttackHitSimple} (miss, shield, crit, damage) with one script per case</li>
 * <li>{@code formulas_attack_timing.json}: {@code calculateTimeBetweenAttacks}, hit time, {@code calculateReuseTime}, {@code calcPAtkSpd}, {@code calcAtkSpd}</li>
 * <li>{@code formulas_atk_break.json}: {@code calcAtkBreak}</li>
 * <li>{@code formulas_regen.json}: {@code calcHpRegen}, {@code calcMpRegen}, {@code calcCpRegen}</li>
 * <li>{@code formulas_lvl_bonus.json}: {@code calcLvlBonusMod}</li>
 * </ul>
 * Needs the game server root as the working directory (see {@link Bootstrap}).
 */
public final class FormulaVectors
{
	private FormulaVectors()
	{
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Creatures and pairs
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	/** name -> spec, in a fixed order. */
	private static final Map<String, StatVectors.Spec> SPECS = new LinkedHashMap<>();
	private static final Map<String, StatVectors.Built> BUILT = new LinkedHashMap<>();

	static
	{
		SPECS.put("duelist76", StatVectors.Spec.player("duelist76", 88, false, 76, StatVectors.enchanted(StatVectors.GEAR_HEAVY_A, 6), true));
		SPECS.put("gladiator40", StatVectors.Spec.player("gladiator40", 2, false, 40, StatVectors.enchanted(StatVectors.GEAR_HEAVY_A, 0), true));
		SPECS.put("archmage76", StatVectors.Spec.player("archmage76", 94, false, 76, StatVectors.enchanted(StatVectors.GEAR_ROBE_A, 3), true));
		SPECS.put("sagittarius76", StatVectors.Spec.player("sagittarius76", 92, false, 76, StatVectors.enchanted(StatVectors.GEAR_LIGHT_BOW_A, 0), true));
		SPECS.put("newbie1", StatVectors.Spec.player("newbie1", 0, false, 1, new int[0][], false));
		SPECS.put("warrior20", StatVectors.Spec.player("warrior20", 1, false, 20, new int[0][], true));
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
	}

	/** {@code attacker, target} pairs: player vs monster at many level differences, monster vs player, player vs player. */
	private static final String[][] PAIRS =
	{
		// Player against monsters. Level penalties start at target level 78 (MinNPCLevelForDmgPenalty), so 18120/18366 are in.
		{
			"newbie1",
			"npc_20001"
		},
		{
			"newbie1",
			"npc_20103"
		},
		{
			"warrior20",
			"npc_20103"
		},
		{
			"warrior20",
			"npc_20201"
		},
		{
			"gladiator40",
			"npc_20239"
		},
		{
			"gladiator40",
			"npc_20677"
		},
		{
			"duelist76",
			"npc_20239"
		},
		{
			"duelist76",
			"npc_20677"
		},
		{
			"duelist76",
			"npc_21090"
		},
		{
			"duelist76",
			"npc_18120"
		},
		{
			"duelist76",
			"npc_18366"
		},
		{
			"duelist76",
			"npc_25010"
		},
		{
			"duelist76",
			"npc_31033"
		},
		{
			"sagittarius76",
			"npc_21090"
		},
		{
			"sagittarius76",
			"npc_18366"
		},
		{
			"archmage76",
			"npc_20677"
		},
		// Monsters against players.
		{
			"npc_20001",
			"newbie1"
		},
		{
			"npc_20103",
			"warrior20"
		},
		{
			"npc_20201",
			"warrior20"
		},
		{
			"npc_20239",
			"gladiator40"
		},
		{
			"npc_20677",
			"duelist76"
		},
		{
			"npc_21090",
			"duelist76"
		},
		{
			"npc_18366",
			"duelist76"
		},
		{
			"npc_25010",
			"duelist76"
		},
		{
			"npc_21001",
			"sagittarius76"
		},
		{
			"npc_31033",
			"archmage76"
		},
		// Player against player.
		{
			"duelist76",
			"gladiator40"
		},
		{
			"gladiator40",
			"duelist76"
		},
		{
			"sagittarius76",
			"archmage76"
		},
		{
			"archmage76",
			"duelist76"
		},
	};

	private static Creature creature(String name)
	{
		return BUILT.get(name).creature;
	}

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Geometry
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	private static final int TARGET_X = 100000;
	private static final int TARGET_Y = 100000;
	private static final int TARGET_Z = 0;
	private static final int TARGET_HEADING = 16384;
	/** relation -> "x,y" of the attacker (heading 0, z = target z + z_diff). */
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
		g.put("note", "The target stands at 'target'. The attacker has heading 0 and z = target z + z_diff, and stands at attacker_xy_by_relation[relation] (found by trying 32 spots 40 units away until the real isInFrontOf / isOnSideOf / isBehind agree).");
		return g;
	}

	/**
	 * Puts the target at a fixed spot and the attacker 40 units away on the side where the real {@code isInFrontOf / isOnSideOf / isBehind} give {@code relation} ("front", "side", "back").
	 * Creature.setXYZ would go through ZoneManager; the coordinates are written straight into the object's own location instead.
	 * @param zDiff attacker z minus target z
	 */
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
				// The spot depends only on the geometry, never on the creatures: every call must find the same one.
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

	/** The result of one scripted call. */
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

	/** The consumed script and the recorded draws of a run, as JSON fields of a case. */
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

	/** A script of {@code n} copies of {@code u}. */
	private static double[] constant(double u, int n)
	{
		final double[] s = new double[n];
		Arrays.fill(s, u);
		return s;
	}

	/** A script of {@code n} values from a fixed-seed generator. */
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

	/** The unit value that makes {@code Rnd.get(1000)} return {@code d}. */
	private static double draw1000(int d)
	{
		return (Math.max(0, Math.min(999, d)) + 0.5) / 1000.0;
	}

	private static final double[] SWEEP =
	{
		0.0,
		0.04,
		0.1,
		0.2,
		0.3,
		0.45,
		0.6,
		0.8,
		0.95,
		0.999
	};

	// -----------------------------------------------------------------------------------------------------------------------------------------------------
	// Files
	// -----------------------------------------------------------------------------------------------------------------------------------------------------

	private static Map<String, Object> header(String file, String source, String note)
	{
		final Map<String, Object> root = Json.header(source);
		root.put("note", note);
		root.put("creatures_file", "formulas_creatures.json");
		return root;
	}

	private static final String RND_NOTE = "Every case has 'script' (the unit values u in [0,1) that were consumed) and 'rnd' (every Rnd call in order: method, bound, returned value; "
		+ "get_int bound b returns floor(u*b), get_int_range o,b returns o+floor(u*(b-o+1)), nextBoolean is u>=0.5). Calls that draw nothing (get with bound <= 0, get(origin, bound) with origin >= bound) are not recorded. Replay the returned values in order and check method and bound.";

	private static String pairName(String[] pair)
	{
		return pair[0] + ">" + pair[1];
	}

	/** Common fields of a pair case. */
	private static Map<String, Object> pairCase(String id, String[] pair, Map<String, Object> placement)
	{
		final Map<String, Object> c = Json.obj();
		c.put("id", id);
		c.put("attacker", pair[0]);
		c.put("target", pair[1]);
		c.put("placement", Json.compact(placement));
		return c;
	}

	private static void write(Path out, String file, Map<String, Object> root, List<Object> cases) throws Exception
	{
		root.put("geometry", geometry());
		writePlain(out, file, root, cases);
	}

	private static void writePlain(Path out, String file, Map<String, Object> root, List<Object> cases) throws Exception
	{
		root.put("cases", cases);
		Json.write(out.resolve(file), root);
		System.out.println("formulas: wrote " + file + " (" + cases.size() + " cases)");
	}

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

		writeCreatures(outDir);
		writeHitMiss(outDir);
		writeCrit(outDir);
		writeShield(outDir);
		writePhysDam(outDir);
		writeAttackHit(outDir);
		writeTiming(outDir);
		writeAtkBreak(outDir);
		writeRegen(outDir);
		writeLvlBonus(outDir);
		System.out.println("formulas: database access answered by the harness fake: " + DatabaseFactory.connections() + " fake connections, " + DatabaseFactory.statements().size() + " distinct statements");
	}

	// -- creatures + config --------------------------------------------------------------------------------------------------------------------------------

	private static void writeCreatures(Path out) throws Exception
	{
		final Map<String, Object> root = header("formulas_creatures.json", "Creatures used by the formula vectors (built by StatVectors) and the config values the formulas read", "Each creature: input (what to build, same shape as stats_*.json) and expected (user_info and raw, same shape). Position and heading are per case.");
		final Map<String, Object> config = Json.obj();
		config.put("AltShieldBlocks", PlayerConfig.ALT_GAME_SHIELD_BLOCKS);
		config.put("AltPerfectShieldBlockRate", PlayerConfig.ALT_PERFECT_SHLD_BLOCK);
		config.put("RandomizeAutoAttackDamage", PlayerConfig.RANDOMIZE_AUTO_ATTACK_DAMAGE);
		config.put("RandomizePhysicalSkillDamage", PlayerConfig.RANDOMIZE_PHYSICAL_SKILL_DAMAGE);
		config.put("AltGameCancelByHit_cast", PlayerConfig.ALT_GAME_CANCEL_CAST);
		config.put("AltGameCancelByHit_bow", PlayerConfig.ALT_GAME_CANCEL_BOW);
		config.put("HpRegenMultiplier", PlayerConfig.HP_REGEN_MULTIPLIER);
		config.put("MpRegenMultiplier", PlayerConfig.MP_REGEN_MULTIPLIER);
		config.put("CpRegenMultiplier", PlayerConfig.CP_REGEN_MULTIPLIER);
		config.put("RaidHpRegenMultiplier", NpcConfig.RAID_HP_REGEN_MULTIPLIER);
		config.put("RaidMpRegenMultiplier", NpcConfig.RAID_MP_REGEN_MULTIPLIER);
		config.put("MinNPCLevelForDmgPenalty", NpcConfig.MIN_NPC_LEVEL_DMG_PENALTY);
		config.put("NpcDmgPenalty", Json.compact(floats(NpcConfig.NPC_DMG_PENALTY)));
		config.put("NpcCritDmgPenalty", Json.compact(floats(NpcConfig.NPC_CRIT_DMG_PENALTY)));
		config.put("NpcSkillDmgPenalty", Json.compact(floats(NpcConfig.NPC_SKILL_DMG_PENALTY)));
		for (String name : new String[]
		{
			"frontBonus",
			"sideBonus",
			"backBonus",
			"highBonus",
			"lowBonus",
			"darkBonus"
		})
		{
			final Field f = HitConditionBonusData.class.getDeclaredField(name);
			f.setAccessible(true);
			config.put("hitConditionBonus_" + name, f.getInt(HitConditionBonusData.getInstance()));
		}

		// ClassBalance: every multiplier array must be 1.0 for the vectors to be valid without it.
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
			final Map<String, Object> c = Json.obj();
			c.put("name", e.getKey());
			c.put("input", StatVectors.input(e.getValue()));
			c.put("expected", StatVectors.dump(e.getValue().creature));
			cases.add(c);
		}

		root.remove("creatures_file");
		root.put("creatures", cases);
		final List<Object> pairs = new ArrayList<>();
		for (String[] pair : PAIRS)
		{
			pairs.add(Json.compact(Arrays.asList(pair)));
		}

		root.put("pairs", pairs);
		Json.write(out.resolve("formulas_creatures.json"), root);
		System.out.println("formulas: wrote formulas_creatures.json (" + cases.size() + " creatures, " + PAIRS.length + " pairs)");
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

	// -- hit / miss ---------------------------------------------------------------------------------------------------------------------------------------

	private static void writeHitMiss(Path out) throws Exception
	{
		final Map<String, Object> root = header("formulas_hit_miss.json", "Formulas.calcHitMiss(attacker, target) (HitConditionBonusData: side, height, night)",
			"result = true means the attack MISSED. " + RND_NOTE + " intermediate.chance_estimate is the generator's own copy of the integer chance for choosing boundary draws; the real result is the Java one. ");
		final List<Object> cases = new ArrayList<>();
		final GameTimeTaskManager clock = GameTimeTaskManager.getInstance();
		for (String[] pair : PAIRS)
		{
			final Creature a = creature(pair[0]);
			final Creature t = creature(pair[1]);
			// Sides at day, plus night and high/low ground on every pair.
			final Object[][] variants =
			{
				{
					"front",
					0,
					false
				},
				{
					"side",
					0,
					false
				},
				{
					"back",
					0,
					false
				},
				{
					"front",
					0,
					true
				},
				{
					"back",
					80,
					false
				},
				{
					"side",
					-80,
					true
				},
			};
			for (Object[] v : variants)
			{
				final Map<String, Object> placement = place(a, t, (String) v[0], (Integer) v[1]);
				clock.setNight((Boolean) v[2]);
				final int accuracy = a.getAccuracy();
				final int evasion = t.getEvasionRate(a);
				final double bonus = HitConditionBonusData.getInstance().getConditionBonus(a, t);
				int chance = (80 + (2 * (accuracy - evasion))) * 10;
				chance *= bonus;
				chance = Math.min(Math.max(chance, 200), 980);
				final double[] draws =
				{
					draw1000(0),
					draw1000(chance - 1),
					draw1000(chance),
					draw1000(chance + 1),
					draw1000(999),
					0.5
				};
				for (int i = 0; i < draws.length; i++)
				{
					final Run r = run(new double[]
					{
						draws[i]
					}, () -> Formulas.calcHitMiss(a, t));
					final Map<String, Object> c = pairCase("hit_" + pairName(pair) + "_" + v[0] + "_z" + v[1] + (((Boolean) v[2]) ? "_night" : "_day") + "_d" + i, pair, placement);
					c.put("night", v[2]);
					final Map<String, Object> inter = Json.obj();
					inter.put("accuracy", accuracy);
					inter.put("target_evasion", evasion);
					inter.put("condition_bonus", bonus);
					inter.put("chance_estimate", chance);
					c.put("intermediate", Json.compact(inter));
					putDraws(c, r);
					c.put("result", r.result);
					cases.add(c);
				}
			}
		}

		clock.setNight(false);
		write(out, "formulas_hit_miss.json", root, cases);
	}

	// -- crit -----------------------------------------------------------------------------------------------------------------------------------------------

	private static void writeCrit(Path out) throws Exception
	{
		final Map<String, Object> root = header("formulas_crit.json", "Formulas.calcCrit(attacker, target) (normal attack)", "result = true means a critical hit. " + RND_NOTE + " intermediate.rate is the generator's own copy of the rate before the target's defence stats.");
		final List<Object> cases = new ArrayList<>();
		for (String[] pair : PAIRS)
		{
			final Creature a = creature(pair[0]);
			final Creature t = creature(pair[1]);
			final Map<String, Object> placement = place(a, t, "front", 0);
			final double rate = a.getStat().calcStat(Stat.CRITICAL_RATE_POS, a.getStat().getCriticalHit(t, null), t, null);
			final double limit = t.getStat().calcStat(Stat.DEFENCE_CRITICAL_RATE, rate, null, null) + t.getStat().calcStat(Stat.DEFENCE_CRITICAL_RATE_ADD, 0, null, null);
			final List<Double> draws = new ArrayList<>();
			for (double u : SWEEP)
			{
				draws.add(u);
			}

			draws.add(draw1000((int) limit - 1));
			draws.add(draw1000((int) limit));
			draws.add(draw1000((int) limit + 1));
			for (int i = 0; i < draws.size(); i++)
			{
				final double u = draws.get(i);
				final Run r = run(new double[]
				{
					u
				}, () -> Formulas.calcCrit(a, t));
				final Map<String, Object> c = pairCase("crit_" + pairName(pair) + "_d" + i, pair, placement);
				final Map<String, Object> inter = Json.obj();
				inter.put("rate", rate);
				inter.put("limit", limit);
				c.put("intermediate", Json.compact(inter));
				putDraws(c, r);
				c.put("result", r.result);
				cases.add(c);
			}
		}

		write(out, "formulas_crit.json", root, cases);
	}

	// -- shield ---------------------------------------------------------------------------------------------------------------------------------------------

	private static void writeShield(Path out) throws Exception
	{
		final Map<String, Object> root = header("formulas_shield.json", "Formulas.calcShldUse(attacker, target) (sendSysMsg true, as in doAttackHitSimple)",
			"result: 0 = no shield defence, 1 = shield defence, 2 = perfect block. Targets without a shield return 0 without drawing. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final double[] first =
		{
			0.0,
			0.5,
			0.97,
			0.999
		};
		final double[] second =
		{
			0.0,
			0.1,
			0.3,
			0.6,
			0.95
		};
		for (String[] pair : PAIRS)
		{
			final Creature a = creature(pair[0]);
			final Creature t = creature(pair[1]);
			final boolean hasShield = t.getSecondaryWeaponItem() != null;
			for (String relation : new String[]
			{
				"front",
				"back"
			})
			{
				final Map<String, Object> placement = place(a, t, relation, 0);
				int n = 0;
				for (double u1 : first)
				{
					for (double u2 : second)
					{
						if (!hasShield && (n > 0))
						{
							continue;
						}

						final Run r = run(new double[]
						{
							u1,
							u2
						}, () -> Formulas.calcShldUse(a, t));
						final Map<String, Object> c = pairCase("shield_" + pairName(pair) + "_" + relation + "_" + n++, pair, placement);
						c.put("target_has_shield", hasShield);
						c.put("target_shield_rate", t.calcStat(Stat.SHIELD_RATE, 0, a, null));
						putDraws(c, r);
						c.put("result", ((Byte) r.result).intValue());
						cases.add(c);
					}
				}
			}
		}

		write(out, "formulas_shield.json", root, cases);
	}

	// -- physical damage -----------------------------------------------------------------------------------------------------------------------------------

	private static void writePhysDam(Path out) throws Exception
	{
		final Map<String, Object> root = header("formulas_phys_dam.json", "Formulas.calcPhysDam(attacker, target, null, shld, crit, ss) (normal attack)",
			"result = damage as a double (before the (int) cast in doAttackHitSimple). shld 2 returns 1 before any draw. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final double[][] scripts =
		{
			constant(0.0, 4),
			constant(0.5, 4),
			constant(0.999, 4)
		};
		for (int p = 0; p < PAIRS.length; p++)
		{
			final String[] pair = PAIRS[p];
			final Creature a = creature(pair[0]);
			final Creature t = creature(pair[1]);
			// Position bonus only matters for a few pairs; the others are measured from the front.
			final String[] relations = p < 6 ? new String[]
			{
				"front",
				"side",
				"back"
			} : new String[]
			{
				"front"
			};
			for (String relation : relations)
			{
				final Map<String, Object> placement = place(a, t, relation, 0);
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
							if ((shld == 2) && (s != 0))
							{
								continue;
							}

							final byte shield = (byte) shld;
							final Run r = run(scripts[s], () -> Formulas.calcPhysDam(a, t, null, shield, crit, ss));
							final Map<String, Object> c = pairCase("pdam_" + pairName(pair) + "_" + relation + "_s" + shld + (crit ? "_crit" : "_nocrit") + (ss ? "_ss" : "_noss") + "_r" + s, pair, placement);
							c.put("shld", shld);
							c.put("crit", crit);
							c.put("ss", ss);
							final Map<String, Object> inter = Json.obj();
							inter.put("p_atk", a.getPAtk(t));
							inter.put("p_def", t.getPDef(a));
							inter.put("shield_def", t.getShldDef());
							inter.put("attacker_level", (int) a.getLevel());
							inter.put("target_level", (int) t.getLevel());
							c.put("intermediate", Json.compact(inter));
							putDraws(c, r);
							c.put("result", r.result);
							cases.add(c);
						}
					}
				}
			}
		}

		write(out, "formulas_phys_dam.json", root, cases);
	}

	// -- one whole normal attack hit ----------------------------------------------------------------------------------------------------------------------

	private static void writeAttackHit(Path out) throws Exception
	{
		final Map<String, Object> root = header("formulas_attack_hit.json", "Creature.doAttackHitSimple order: calcHitMiss, then (if not missed) calcShldUse, calcCrit and (int) calcPhysDam",
			"The four calls are made in the order doAttackHitSimple makes them, with one script per case, so the order of draws is pinned. result = { miss, shld, crit, damage (int, 0 when missed) }. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		for (int p = 0; p < PAIRS.length; p++)
		{
			final String[] pair = PAIRS[p];
			final Creature a = creature(pair[0]);
			final Creature t = creature(pair[1]);
			final Map<String, Object> placement = place(a, t, "front", 0);
			for (int s = 0; s < 8; s++)
			{
				final boolean ss = (s & 1) != 0;
				final double[] script = seeded(0xA77AC4L + (p * 100L) + s, 12);
				final Run r = run(script, () ->
				{
					final Map<String, Object> res = Json.obj();
					final boolean miss = Formulas.calcHitMiss(a, t);
					byte shld = 0;
					boolean crit = false;
					int damage = 0;
					if (!miss)
					{
						shld = Formulas.calcShldUse(a, t);
						crit = Formulas.calcCrit(a, t);
						damage = (int) Formulas.calcPhysDam(a, t, null, shld, crit, ss);
					}

					res.put("miss", miss);
					res.put("shld", (int) shld);
					res.put("crit", crit);
					res.put("damage", damage);
					return res;
				});
				final Map<String, Object> c = pairCase("hit_seq_" + pairName(pair) + "_" + s, pair, placement);
				c.put("ss", ss);
				putDraws(c, r);
				c.put("result", Json.compact(r.result));
				cases.add(c);
			}
		}

		write(out, "formulas_attack_hit.json", root, cases);
	}

	// -- attack timing -------------------------------------------------------------------------------------------------------------------------------------

	private static void writeTiming(Path out) throws Exception
	{
		final Map<String, Object> root = header("formulas_attack_timing.json", "Creature.calculateTimeBetweenAttacks / calculateReuseTime (doAttack), Formulas.calcPAtkSpd, Formulas.calcAtkSpd",
			"creatures: per creature the values doAttack uses: time_atk = (int)(500000 / pAtkSpd), time_to_hit = time_atk / 2, reuse = calculateReuseTime(active weapon). "
				+ "calc_p_atk_spd: Formulas.calcPAtkSpd(attacker, target, rate) -> ms. calc_atk_spd: Formulas.calcAtkSpd(attacker, skill, skillTime) for a magic and a physical skill.");
		final List<Object> creatures = new ArrayList<>();
		for (Map.Entry<String, StatVectors.Built> e : BUILT.entrySet())
		{
			final Creature c = e.getValue().creature;
			final Weapon weapon = c.getActiveWeaponItem();
			final Map<String, Object> m = Json.obj();
			m.put("creature", e.getKey());
			m.put("p_atk_spd", c.getPAtkSpd());
			m.put("m_atk_spd", c.getMAtkSpd());
			m.put("attack_type", c.getAttackType().name());
			m.put("weapon_id", weapon == null ? 0 : weapon.getId());
			m.put("weapon_reuse_delay", weapon == null ? 0 : weapon.getReuseDelay());
			m.put("time_atk", c.calculateTimeBetweenAttacks());
			m.put("time_to_hit", c.calculateTimeBetweenAttacks() / 2);
			m.put("reuse", c.calculateReuseTime(weapon));
			creatures.add(Json.compact(m));
		}

		root.put("creatures", creatures);

		final double[] rates =
		{
			-5,
			0,
			1,
			1.999,
			2,
			3,
			100,
			181.5,
			278,
			330,
			417.25,
			500,
			666,
			1000,
			1e6
		};
		final List<Object> calc = new ArrayList<>();
		final Creature any = creature("duelist76");
		for (double rate : rates)
		{
			final Map<String, Object> m = Json.obj();
			m.put("rate", rate);
			m.put("result", Formulas.calcPAtkSpd(any, any, rate));
			calc.add(Json.compact(m));
		}

		root.put("calc_p_atk_spd", calc);

		final List<Object> skillCalc = new ArrayList<>();
		final Skill magic = SkillData.getInstance().getSkill(1069, 1);
		final Skill physical = SkillData.getInstance().getSkill(4, 1);
		for (String name : new String[]
		{
			"duelist76",
			"archmage76",
			"sagittarius76",
			"npc_20677"
		})
		{
			final Creature c = creature(name);
			for (Skill skill : new Skill[]
			{
				magic,
				physical
			})
			{
				for (double time : new double[]
				{
					500,
					1000,
					2000,
					6000
				})
				{
					final Map<String, Object> m = Json.obj();
					m.put("creature", name);
					m.put("skill_id", skill.getId());
					m.put("skill_level", skill.getLevel());
					m.put("skill_is_magic", skill.isMagic());
					m.put("skill_time", time);
					m.put("m_atk_spd", c.getMAtkSpd());
					m.put("p_atk_spd", c.getPAtkSpd());
					m.put("result", Formulas.calcAtkSpd(c, skill, time));
					skillCalc.add(Json.compact(m));
				}
			}
		}

		root.put("calc_atk_spd", skillCalc);
		Json.write(out.resolve("formulas_attack_timing.json"), root);
		System.out.println("formulas: wrote formulas_attack_timing.json (" + creatures.size() + " creatures, " + calc.size() + " + " + skillCalc.size() + " calcs)");
	}

	// -- attack break --------------------------------------------------------------------------------------------------------------------------------------

	private static void writeAtkBreak(Path out) throws Exception
	{
		final Map<String, Object> root = header("formulas_atk_break.json", "Formulas.calcAtkBreak(target, damage)",
			"state: 'cast' = target.setCastingNow(true), 'bow_attack' = target is attacking now with a bow (attack end time in the future), 'idle' = neither (returns false without a draw). "
				+ "Needs AltGameCancelByHit (config) to include cast / bow. " + RND_NOTE);
		final List<Object> cases = new ArrayList<>();
		final Field endTime = Creature.class.getDeclaredField("_attackEndTime");
		endTime.setAccessible(true);
		final double[] damages =
		{
			1,
			25,
			100,
			400,
			1500,
			8000
		};
		final double[][] scripts =
		{
			constant(0.0, 2),
			constant(0.5, 2),
			constant(0.99, 2)
		};
		for (String name : new String[]
		{
			"duelist76",
			"archmage76",
			"sagittarius76",
			"npc_20677",
			"npc_25010",
			"npc_31033"
		})
		{
			final Creature t = creature(name);
			for (String state : new String[]
			{
				"idle",
				"cast",
				"bow_attack"
			})
			{
				if (state.equals("bow_attack") && !name.equals("sagittarius76"))
				{
					continue;
				}

				t.setCastingNow(state.equals("cast"));
				endTime.setLong(t, state.equals("bow_attack") ? System.nanoTime() + (3_600_000_000_000L) : 0L);
				for (double dmg : damages)
				{
					for (int s = 0; s < scripts.length; s++)
					{
						if (state.equals("idle") && (s > 0))
						{
							continue;
						}

						final Run r = run(scripts[s], () -> Formulas.calcAtkBreak(t, dmg));
						final Map<String, Object> c = Json.obj();
						c.put("id", "atkbreak_" + name + "_" + state + "_d" + (int) dmg + "_r" + s);
						c.put("target", name);
						c.put("state", state);
						c.put("damage", dmg);
						final Map<String, Object> inter = Json.obj();
						inter.put("men_bonus", org.l2jmobius.gameserver.mechanics.stats.BaseStat.MEN.calcBonus(t));
						inter.put("attack_cancel_probe", t.calcStat(Stat.ATTACK_CANCEL, 50, null, null));
						inter.put("is_raid", t.isRaid());
						inter.put("is_invul", t.isInvul());
						c.put("intermediate", Json.compact(inter));
						putDraws(c, r);
						c.put("result", r.result);
						cases.add(c);
					}
				}
			}

			t.setCastingNow(false);
			endTime.setLong(t, 0L);
		}

		write(out, "formulas_atk_break.json", root, cases);
	}

	// -- regeneration --------------------------------------------------------------------------------------------------------------------------------------

	private static void writeRegen(Path out) throws Exception
	{
		final Map<String, Object> root = header("formulas_regen.json", "Formulas.calcHpRegen / calcMpRegen / calcCpRegen",
			"Per creature and movement state: 'stand' (not moving, not sitting), 'sit', 'move_run', 'move_walk'. No draws. hp/mp/cp are the returned rates (per 3 s regeneration period). cp is only for players. "
				+ "Zones, clan hall / castle / fort functions, festival and siege modifiers are not covered (no zones loaded, no clan).");
		final List<Object> cases = new ArrayList<>();
		final Field move = Creature.class.getDeclaredField("_move");
		move.setAccessible(true);
		final Field running = Creature.class.getDeclaredField("_isRunning");
		running.setAccessible(true);

		// The formula creatures in all four states.
		for (Map.Entry<String, StatVectors.Built> e : BUILT.entrySet())
		{
			final Creature c = e.getValue().creature;
			for (String state : new String[]
			{
				"stand",
				"sit",
				"move_run",
				"move_walk"
			})
			{
				if (!c.isPlayer() && state.equals("sit"))
				{
					continue;
				}

				cases.add(regenCase(e.getKey(), null, c, state, move, running));
			}
		}

		// Every stat case creature, standing.
		final List<StatVectors.Spec> specs = StatVectors.allSpecs();
		for (StatVectors.Spec spec : specs)
		{
			final StatVectors.Built built = StatVectors.build(spec);
			if (built == null)
			{
				continue;
			}

			cases.add(regenCase("stat_" + spec.id, spec, built.creature, "stand", move, running));
		}

		writePlain(out, "formulas_regen.json", root, cases);
	}

	private static Object regenCase(String id, StatVectors.Spec spec, Creature c, String state, Field move, Field running) throws Exception
	{
		final Player player = c.isPlayer() ? c.asPlayer() : null;
		if (player != null)
		{
			player.setSitting(state.equals("sit"));
		}

		move.set(c, (state.startsWith("move")) ? new Creature.MoveData() : null);
		running.setBoolean(c, state.equals("move_run") || state.equals("stand"));

		final Map<String, Object> m = Json.obj();
		m.put("id", id + "_" + state);
		m.put("creature", id);
		m.put("state", state);
		m.put("is_raid", c.isRaid());
		m.put("level", (int) c.getLevel());
		m.put("hp", Formulas.calcHpRegen(c));
		m.put("mp", Formulas.calcMpRegen(c));
		if (player != null)
		{
			m.put("cp", Formulas.calcCpRegen(player));
		}

		if (player != null)
		{
			player.setSitting(false);
		}

		move.set(c, null);
		running.setBoolean(c, false);
		return Json.compact(m);
	}

	// -- level bonus ---------------------------------------------------------------------------------------------------------------------------------------

	private static void writeLvlBonus(Path out) throws Exception
	{
		final Map<String, Object> root = header("formulas_lvl_bonus.json", "Formulas.calcLvlBonusMod(attacker, target, skill)",
			"result = (1 + lvlBonusRate / 100) * (1 + (attackerLevel - targetLevel) / 100) where attackerLevel is the skill's magic level when it is above 0, else the attacker's level. No draws.");
		final List<Object> cases = new ArrayList<>();
		final int[][] skills =
		{
			{
				1069,
				1
			},
			{
				1064,
				2
			},
			{
				1095,
				1
			},
			{
				1169,
				3
			},
			{
				1263,
				3
			},
			{
				1337,
				1
			},
			{
				1297,
				1
			},
			{
				4,
				1
			},
			{
				1108,
				1
			},
		};
		for (String[] pair : PAIRS)
		{
			final Creature a = creature(pair[0]);
			final Creature t = creature(pair[1]);
			for (int[] s : skills)
			{
				final Skill skill = SkillData.getInstance().getSkill(s[0], s[1]);
				if (skill == null)
				{
					throw new IllegalStateException("skill " + s[0] + ":" + s[1] + " missing");
				}

				final Map<String, Object> m = Json.obj();
				m.put("id", "lvlbonus_" + pairName(pair) + "_" + s[0] + "_" + s[1]);
				m.put("attacker", pair[0]);
				m.put("target", pair[1]);
				m.put("skill_id", s[0]);
				m.put("skill_level", s[1]);
				m.put("skill_magic_level", skill.getMagicLevel());
				m.put("skill_lvl_bonus_rate", skill.getLvlBonusRate());
				m.put("attacker_level", (int) a.getLevel());
				m.put("target_level", (int) t.getLevel());
				m.put("result", Formulas.calcLvlBonusMod(a, t, skill));
				cases.add(Json.compact(m));
			}
		}

		writePlain(out, "formulas_lvl_bonus.json", root, cases);
	}
}
