import HealthKit

/// Every HealthKit type Atlys reads. Identifiers are raw strings so that
/// types missing from the running iOS version are skipped at runtime (the
/// lookup returns nil) instead of failing the build.
enum HealthTypes {
    static let quantityIdentifiers: [String] = [
        // Body
        "BodyMassIndex", "BodyFatPercentage", "Height", "BodyMass", "LeanBodyMass", "WaistCircumference",
        "AppleSleepingWristTemperature",
        // Activity and fitness
        "StepCount", "DistanceWalkingRunning", "DistanceCycling", "DistanceWheelchair", "BasalEnergyBurned",
        "ActiveEnergyBurned", "FlightsClimbed", "NikeFuel", "AppleExerciseTime", "PushCount", "DistanceSwimming",
        "SwimmingStrokeCount", "VO2Max", "DistanceDownhillSnowSports", "AppleStandTime", "AppleMoveTime",
        "PhysicalEffort", "TimeInDaylight", "DistanceRowing", "RowingSpeed", "DistancePaddleSports",
        "PaddleSportsSpeed", "DistanceCrossCountrySkiing", "CrossCountrySkiingSpeed", "DistanceSkatingSports",
        "SkatingSportsSpeed", "EstimatedWorkoutEffortScore", "WorkoutEffortScore",
        // Mobility
        "WalkingSpeed", "WalkingDoubleSupportPercentage", "WalkingAsymmetryPercentage", "WalkingStepLength",
        "SixMinuteWalkTestDistance", "StairAscentSpeed", "StairDescentSpeed", "AppleWalkingSteadiness",
        // Running and cycling
        "RunningStrideLength", "RunningVerticalOscillation", "RunningGroundContactTime", "RunningPower",
        "RunningSpeed", "CyclingSpeed", "CyclingPower", "CyclingFunctionalThresholdPower", "CyclingCadence",
        // Vitals and heart
        "HeartRate", "BodyTemperature", "BasalBodyTemperature", "BloodPressureSystolic", "BloodPressureDiastolic",
        "RespiratoryRate", "RestingHeartRate", "WalkingHeartRateAverage", "HeartRateVariabilitySDNN",
        "HeartRateRecoveryOneMinute", "AtrialFibrillationBurden", "OxygenSaturation", "PeripheralPerfusionIndex",
        "AppleSleepingBreathingDisturbances",
        // Lab and other results
        "BloodGlucose", "NumberOfTimesFallen", "ElectrodermalActivity", "InhalerUsage", "InsulinDelivery",
        "BloodAlcoholContent", "ForcedVitalCapacity", "ForcedExpiratoryVolume1", "PeakExpiratoryFlowRate",
        "NumberOfAlcoholicBeverages",
        // Hearing and environment
        "EnvironmentalAudioExposure", "HeadphoneAudioExposure", "EnvironmentalSoundReduction", "UVExposure",
        "UnderwaterDepth", "WaterTemperature",
        // Nutrition
        "DietaryFatTotal", "DietaryFatPolyunsaturated", "DietaryFatMonounsaturated", "DietaryFatSaturated",
        "DietaryCholesterol", "DietarySodium", "DietaryCarbohydrates", "DietaryFiber", "DietarySugar",
        "DietaryEnergyConsumed", "DietaryProtein", "DietaryVitaminA", "DietaryVitaminB6", "DietaryVitaminB12",
        "DietaryVitaminC", "DietaryVitaminD", "DietaryVitaminE", "DietaryVitaminK", "DietaryCalcium",
        "DietaryIron", "DietaryThiamin", "DietaryRiboflavin", "DietaryNiacin", "DietaryFolate", "DietaryBiotin",
        "DietaryPantothenicAcid", "DietaryPhosphorus", "DietaryIodine", "DietaryMagnesium", "DietaryZinc",
        "DietarySelenium", "DietaryCopper", "DietaryManganese", "DietaryChromium", "DietaryMolybdenum",
        "DietaryChloride", "DietaryPotassium", "DietaryCaffeine", "DietaryWater",
    ].map { "HKQuantityTypeIdentifier" + $0 }

    static let categoryIdentifiers: [String] = [
        "SleepAnalysis", "AppleStandHour", "CervicalMucusQuality", "OvulationTestResult", "PregnancyTestResult",
        "ProgesteroneTestResult", "MenstrualFlow", "IntermenstrualBleeding", "PersistentIntermenstrualBleeding",
        "ProlongedMenstrualPeriods", "IrregularMenstrualCycles", "InfrequentMenstrualCycles", "SexualActivity",
        "Contraceptive", "Pregnancy", "Lactation", "BleedingDuringPregnancy", "BleedingAfterPregnancy",
        "MindfulSession", "HighHeartRateEvent", "LowHeartRateEvent", "IrregularHeartRhythmEvent",
        "LowCardioFitnessEvent", "AudioExposureEvent", "EnvironmentalAudioExposureEvent",
        "HeadphoneAudioExposureEvent", "ToothbrushingEvent", "HandwashingEvent", "AppleWalkingSteadinessEvent",
        "SleepApneaEvent", "HypertensionEvent",
        // Symptoms
        "AbdominalCramps", "Acne", "AppetiteChanges", "BladderIncontinence", "Bloating", "BreastPain",
        "ChestTightnessOrPain", "Chills", "Constipation", "Coughing", "Diarrhea", "Dizziness", "DrySkin",
        "Fainting", "Fatigue", "Fever", "GeneralizedBodyAche", "HairLoss", "Headache", "Heartburn", "HotFlashes",
        "LossOfSmell", "LossOfTaste", "LowerBackPain", "MemoryLapse", "MoodChanges", "Nausea", "NightSweats",
        "PelvicPain", "RapidPoundingOrFlutteringHeartbeat", "RunnyNose", "ShortnessOfBreath", "SinusCongestion",
        "SkippedHeartbeat", "SleepChanges", "SoreThroat", "VaginalDryness", "Vomiting", "Wheezing",
    ].map { "HKCategoryTypeIdentifier" + $0 }

    static let clinicalIdentifiers: [String] = [
        "AllergyRecord", "ClinicalNoteRecord", "ConditionRecord", "ImmunizationRecord", "LabResultRecord",
        "MedicationRecord", "ProcedureRecord", "VitalSignRecord", "CoverageRecord",
    ].map { "HKClinicalTypeIdentifier" + $0 }

    static var quantityTypes: [HKQuantityType] {
        quantityIdentifiers.compactMap { HKQuantityType.quantityType(forIdentifier: .init(rawValue: $0)) }
    }

    static var categoryTypes: [HKCategoryType] {
        categoryIdentifiers.compactMap { HKCategoryType.categoryType(forIdentifier: .init(rawValue: $0)) }
    }

    static var clinicalTypes: [HKClinicalType] {
        clinicalIdentifiers.compactMap { HKClinicalType.clinicalType(forIdentifier: .init(rawValue: $0)) }
    }

    /// Workouts, series, ECG, audiograms, State of Mind, and assessments.
    static var otherSampleTypes: [HKSampleType] {
        [
            HKObjectType.workoutType(),
            HKSeriesType.workoutRoute(),
            HKSeriesType.heartbeat(),
            HKObjectType.electrocardiogramType(),
            HKObjectType.audiogramSampleType(),
            HKSampleType.stateOfMindType(),
            HKScoredAssessmentType(.GAD7),
            HKScoredAssessmentType(.PHQ9),
        ]
    }

    static var characteristicTypes: [HKCharacteristicType] {
        [
            HKCharacteristicTypeIdentifier.biologicalSex, .bloodType, .dateOfBirth, .fitzpatrickSkinType,
            .wheelchairUse, .activityMoveMode,
        ].compactMap { HKCharacteristicType.characteristicType(forIdentifier: $0) }
    }

    /// Sample types read incrementally, in collection order. Routes are read
    /// with their workouts, so they are not listed separately.
    static func sampleTypes(clinical: Bool) -> [HKSampleType] {
        let others = otherSampleTypes.filter { $0 != HKSeriesType.workoutRoute() }
        return quantityTypes + categoryTypes + others + (clinical ? clinicalTypes : [])
    }

    /// Everything to request read access for.
    static func readTypes(clinical: Bool) -> Set<HKObjectType> {
        var types = Set<HKObjectType>(quantityTypes as [HKObjectType])
        types.formUnion(categoryTypes as [HKObjectType])
        types.formUnion(otherSampleTypes as [HKObjectType])
        types.formUnion(characteristicTypes as [HKObjectType])
        if clinical { types.formUnion(clinicalTypes as [HKObjectType]) }
        return types
    }

    /// Units tried, in order, when Apple Health has no preferred unit for a type.
    static let fallbackUnits: [HKUnit] = [
        .count(), .count().unitDivided(by: .minute()), .meter(), .second(), .kilocalorie(),
        .gramUnit(with: .kilo), .percent(), .degreeCelsius(), .millimeterOfMercury(),
        HKUnit(from: "mg/dL"), .liter(), .decibelAWeightedSoundPressureLevel(), .meter().unitDivided(by: .second()),
        .watt(), .secondUnit(with: .milli), .siemen(), .internationalUnit(), HKUnit(from: "ml/(kg*min)"), .gram(),
        .count().unitDivided(by: .second()), .liter().unitDivided(by: .minute()), .lux(), .hertz(), .volt(),
        .degreeAngle(), .decibelHearingLevel(), .appleEffortScore(), HKUnit(from: "kcal/(kg*hr)"),
    ]

    static let sleepNames = ["inBed", "asleepUnspecified", "awake", "asleepCore", "asleepDeep", "asleepREM"]

    static func valueName(identifier: String, value: Int) -> String? {
        switch identifier {
        case "HKCategoryTypeIdentifierSleepAnalysis":
            sleepNames.indices.contains(value) ? sleepNames[value] : nil
        case "HKCategoryTypeIdentifierAppleStandHour":
            value == 0 ? "stood" : value == 1 ? "idle" : nil
        default:
            nil
        }
    }

    static let workoutNames: [UInt: String] = [
        1: "american_football", 2: "archery", 3: "australian_football", 4: "badminton", 5: "baseball",
        6: "basketball", 7: "bowling", 8: "boxing", 9: "climbing", 10: "cricket", 11: "cross_training",
        12: "curling", 13: "cycling", 16: "elliptical", 17: "equestrian_sports", 18: "fencing", 19: "fishing",
        20: "functional_strength_training", 21: "golf", 22: "gymnastics", 23: "handball", 24: "hiking",
        25: "hockey", 26: "hunting", 27: "lacrosse", 28: "martial_arts", 29: "mind_and_body",
        31: "paddle_sports", 32: "play", 33: "preparation_and_recovery", 34: "racquetball", 35: "rowing",
        36: "rugby", 37: "running", 38: "sailing", 39: "skating_sports", 40: "snow_sports", 41: "soccer",
        42: "softball", 43: "squash", 44: "stair_climbing", 45: "surfing_sports", 46: "swimming",
        47: "table_tennis", 48: "tennis", 49: "track_and_field", 50: "traditional_strength_training",
        51: "volleyball", 52: "walking", 53: "water_fitness", 54: "water_polo", 55: "water_sports",
        56: "wrestling", 57: "yoga", 58: "barre", 59: "core_training", 60: "cross_country_skiing",
        61: "downhill_skiing", 62: "flexibility", 63: "high_intensity_interval_training", 64: "jump_rope",
        65: "kickboxing", 66: "pilates", 67: "snowboarding", 68: "stairs", 69: "step_training",
        70: "wheelchair_walk_pace", 71: "wheelchair_run_pace", 72: "tai_chi", 73: "mixed_cardio",
        74: "hand_cycling", 75: "disc_sports", 76: "fitness_gaming", 77: "cardio_dance",
        78: "social_dance", 79: "pickleball", 80: "cooldown", 82: "swim_bike_run", 83: "transition",
        84: "underwater_diving", 3000: "other",
    ]
}
